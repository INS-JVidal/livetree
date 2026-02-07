# LiveTree — Real-Time Directory Tree Watcher

## Project Overview

**LiveTree** is a terminal-based tool that monitors a directory and continuously renders an updated tree view in-place. It uses double-buffered rendering to eliminate screen flicker, providing a smooth, real-time visualization of filesystem changes.

**Target platform:** Linux (with potential cross-platform support via abstracted backends)

---

## Language Choice: Rust

### Decision Matrix

| Criterion               | Rust | C    | C++  | Dart |
|--------------------------|------|------|------|------|
| Filesystem watcher libs  | ★★★★★ | ★★★  | ★★★  | ★★   |
| Terminal UI ecosystem    | ★★★★★ | ★★★  | ★★★  | ★    |
| Memory safety            | ★★★★★ | ★    | ★★   | ★★★★ |
| Single binary output     | ★★★★★ | ★★★★★| ★★★★★| ★★   |
| Developer ergonomics     | ★★★★  | ★★   | ★★★  | ★★★★ |
| Concurrency model        | ★★★★★ | ★★   | ★★★  | ★★★★ |

### Justification

Rust wins on the combination of factors most critical for this project:

- **`notify` crate (v7)** — production-grade filesystem watcher that abstracts over inotify (Linux), kqueue (macOS), and ReadDirectoryChanges (Windows). Includes a built-in debouncer, eliminating a whole class of bugs.
- **`crossterm`** — cross-platform terminal manipulation with raw mode, alternate screen, and buffered writes. This is the foundation for flicker-free double buffering.
- **`walkdir`** — efficient recursive directory traversal with configurable depth, symlink handling, and error reporting per entry.
- **Ownership model** — guarantees no data races between the watcher thread and the renderer, enforced at compile time.
- **Zero-cost abstractions** — no GC pauses during rendering, critical for smooth visual output.

### Why Not the Others

- **C** — No built-in filesystem watcher abstraction; you'd manually manage `inotify` file descriptors, handle buffer parsing, and implement your own recursive watch setup. Terminal buffering requires manual `write()`/`setvbuf()` management. High risk of memory bugs.
- **C++** — Better than C with `std::filesystem`, but still lacks a mature cross-platform watcher library. `ncurses` is powerful but heavyweight for this use case. Manual memory management remains a footgun.
- **Dart** — `dart:io` has `FileSystemEntity.watch()` but it's limited (no recursive watch on all platforms). Terminal UI ecosystem is nearly nonexistent. Requires the Dart runtime.

---

## Architecture

### High-Level Design

```
┌───────────────────────────────────────────────────────┐
│                      Main Thread                      │
│                                                       │
│  ┌─────────┐    ┌─────────────┐    ┌───────────────┐ │
│  │  Init    │───▶│ TreeBuilder │───▶│   Renderer    │ │
│  │ (args,   │    │ (walkdir)   │    │ (crossterm +  │ │
│  │  screen) │    │             │    │  BufWriter)   │ │
│  └─────────┘    └──────▲──────┘    └───────────────┘ │
│                        │                              │
│                 ┌──────┴──────┐                       │
│                 │  Event Loop │◀──── Ctrl+C / 'q'     │
│                 │  (select)   │                       │
│                 └──────▲──────┘                       │
│                        │                              │
│              ┌─────────┴─────────┐                    │
│              │   Watcher Thread  │                    │
│              │  (notify + dedup) │                    │
│              └───────────────────┘                    │
└───────────────────────────────────────────────────────┘
```

### Component Breakdown

#### 1. CLI Interface (`clap`)

```
livetree [OPTIONS] [PATH]

Arguments:
  [PATH]    Directory to watch (default: current directory)

Options:
  -L, --level <DEPTH>     Max display depth (default: unlimited)
  -I, --ignore <PATTERN>  Glob patterns to exclude (repeatable)
  -a, --all               Show hidden files (dotfiles)
  -D, --dirs-only         Only show directories
  -f, --follow-symlinks   Follow symbolic links
  --debounce <MS>         Debounce interval in milliseconds (default: 200)
  --no-color              Disable colored output
  -h, --help              Print help
  -V, --version           Print version
```

#### 2. Filesystem Watcher (`notify` v7)

**Responsibilities:**
- Watch the target directory recursively using `RecommendedWatcher`
- Debounce rapid events (file saves trigger multiple events) using `notify-debouncer-full`
- Send a simple "refresh" signal via `mpsc::channel` — no need to track individual changes

**Key decisions:**
- Use the **full debouncer** (`notify-debouncer-full`) rather than the mini debouncer — it coalesces rename pairs and provides cleaner event streams
- Debounce window: **200ms default** (configurable). This balances responsiveness with avoiding redundant redraws during batch operations like `git checkout`
- The watcher thread only signals "something changed" — the main thread always does a full re-scan. This is simpler and more reliable than incremental tree updates, and for typical directory sizes (< 10K entries) the scan is sub-millisecond

**Edge cases handled:**
- Watched directory itself is deleted → graceful shutdown with message
- Permission denied on subdirectory → skip with warning marker in tree
- Symlink loops → detected and stopped by `walkdir` with `follow_links` + cycle detection

#### 3. Tree Builder (`walkdir`)

**Responsibilities:**
- Walk the directory tree respecting depth limits and ignore patterns
- Build a flat `Vec<TreeEntry>` sorted by path (directories first within each level)
- Calculate tree-drawing prefixes (connectors)

**Data structure:**

```rust
struct TreeEntry {
    name: String,        // filename only
    depth: usize,        // nesting level (0 = root)
    is_dir: bool,
    is_symlink: bool,
    is_last: bool,       // last sibling at this depth
    prefix: String,      // precomputed "│   ├── " string
    error: Option<String>, // permission denied, etc.
}
```

**Sorting strategy:**
- Directories before files at each level (matches `tree` behavior)
- Alphabetical within each group (case-insensitive)
- Dotfiles last (when shown with `--all`)

**Ignore patterns:**
- Default ignores: `.git`, `node_modules`, `__pycache__`, `.DS_Store`
- User-specified via `-I` flag using glob matching (`globset` crate)
- Applied during traversal (not post-filter) for performance

#### 4. Renderer (Double-Buffered)

**This is the core of the flicker-free experience.** The strategy:

```
┌────────────────────────────────────────────┐
│           Rendering Pipeline               │
│                                            │
│  1. Write to BufWriter<Stdout>             │
│     (all output stays in memory)           │
│                                            │
│  2. MoveTo(0,0) — reposition cursor        │
│                                            │
│  3. Write each line of the tree            │
│     (overwrites previous content)          │
│                                            │
│  4. Clear remaining lines from             │
│     previous frame (if tree shrank)        │
│                                            │
│  5. stdout.flush() — single syscall        │
│     pushes entire frame to terminal        │
│                                            │
│  Result: terminal sees one atomic update   │
└────────────────────────────────────────────┘
```

**Double-buffer implementation detail:**

```rust
use std::io::{BufWriter, Write, stdout};
use crossterm::{cursor, terminal, execute, queue};

fn render(writer: &mut BufWriter<Stdout>, entries: &[TreeEntry], prev_lines: usize) {
    // All these writes go to BufWriter's internal buffer (memory)
    queue!(writer, cursor::MoveTo(0, 0)).unwrap();

    let mut current_lines = 0;
    for entry in entries {
        queue!(writer, 
            terminal::Clear(terminal::ClearType::CurrentLine)
        ).unwrap();
        writeln!(writer, "{}{}", entry.prefix, styled_name(entry)).unwrap();
        current_lines += 1;
    }

    // Clear leftover lines from previous render
    for _ in current_lines..prev_lines {
        queue!(writer,
            terminal::Clear(terminal::ClearType::CurrentLine),
            cursor::MoveDown(1)
        ).unwrap();
    }

    // === THE BUFFER SWAP === 
    // Single flush() call writes everything to terminal at once
    writer.flush().unwrap();
}
```

**Why this works:**
- `BufWriter` accumulates all escape sequences and text in a userspace buffer
- The terminal receives one large `write()` syscall instead of hundreds of small ones
- Modern terminal emulators process this as a single "frame", eliminating visible flicker
- No need for alternate screen (which hides scrollback) — we simply overwrite in place

**Additional rendering details:**
- **Status bar** at bottom: shows watched path, entry count, last event timestamp
- **Terminal resize handling** — `crossterm` signals `SIGWINCH`; truncate long lines to terminal width
- **Color scheme** (via `crossterm` styling):
  - Directories: bold blue
  - Symlinks: cyan with `→ target` suffix
  - Executables: bold green
  - Errors: red
  - Tree connectors (`├──`, `└──`, `│`): dim white

#### 5. Event Loop

```rust
fn main_loop() {
    // Channels
    let (fs_tx, fs_rx) = mpsc::channel();  // filesystem events
    let (key_tx, key_rx) = mpsc::channel(); // keyboard input

    // Spawn watcher thread
    let watcher = start_watcher(&path, fs_tx, debounce_ms);

    // Spawn input thread (raw mode keyboard polling)
    thread::spawn(move || {
        loop {
            if let Ok(Event::Key(key)) = crossterm::event::read() {
                key_tx.send(key).ok();
            }
        }
    });

    // Main render loop
    loop {
        select! {
            recv(fs_rx) -> _ => {
                let entries = build_tree(&path, &config);
                render(&mut writer, &entries, prev_lines);
                prev_lines = entries.len();
            }
            recv(key_rx) -> key => {
                match key {
                    'q' | Ctrl+'c' => break,
                    // future: arrow keys for scrolling
                }
            }
        }
    }
}
```

The `select!` macro (from the `crossbeam-channel` crate) allows us to wait on multiple channels simultaneously without busy-looping.

---

## Error Handling Strategy

| Scenario | Behavior |
|----------|----------|
| Target path doesn't exist | Exit with clear error message |
| Target path is a file, not directory | Exit with suggestion to pass parent dir |
| Permission denied on root | Exit with error |
| Permission denied on subdirectory | Show entry with `[permission denied]` marker in red |
| Watched directory deleted | Display message, wait for re-creation or exit |
| Symlink loop detected | Show entry with `[cycle]` marker, don't follow |
| Terminal too narrow | Truncate filenames with `…` |
| Terminal too short | Show as many entries as fit + `... and N more` |
| inotify watch limit reached | Warn user, suggest `sysctl fs.inotify.max_user_watches` |
| `walkdir` I/O error on entry | Skip entry, log to stderr |

---

## Dependencies (`Cargo.toml`)

```toml
[package]
name = "livetree"
version = "0.1.0"
edition = "2021"

[dependencies]
notify = "7"                    # Filesystem watching
notify-debouncer-full = "0.4"   # Event debouncing
walkdir = "2"                   # Recursive directory traversal
crossterm = "0.28"              # Terminal manipulation
crossbeam-channel = "0.5"       # Multi-channel select
clap = { version = "4", features = ["derive"] }  # CLI parsing
globset = "0.4"                 # Ignore patterns
```

Total: **7 direct dependencies**, all well-maintained and widely used in the Rust ecosystem.

---

## Implementation Phases

### Phase 0 — Skeleton (1 hour)

- Project scaffolding with `clap`
- Basic `walkdir` traversal → print tree to stdout (no watching)
- Verify box-drawing characters render correctly

### Phase 1 — Double-Buffered Rendering (1-2 hours)

- Raw mode + `BufWriter<Stdout>`
- Cursor repositioning and in-place overwrite
- Handle terminal resize
- Clean exit on `q` / Ctrl+C (restore terminal state!)

### Phase 2 — Filesystem Watching (1-2 hours)

- Integrate `notify` with debouncer
- Wire events to trigger re-scan + re-render
- Test with rapid file creation/deletion

### Phase 3 — Polish (2-3 hours)

- Color output
- Ignore patterns (default + custom)
- Status bar with metadata
- Depth limiting
- Edge case handling (permissions, symlinks, deleted root)

### Phase 4 — Optional Enhancements

- Scrolling for large trees (arrow keys + page up/down)
- File size display mode
- Git status integration (changed/untracked markers)
- Export snapshot to file
- Configuration file (`~/.config/livetree/config.toml`)

---

## Testing Strategy

| Test Type | Approach |
|-----------|----------|
| Tree building | Unit tests with `tempdir` — create known structures, verify output |
| Rendering | Snapshot tests — capture buffer output, compare against expected |
| Watcher integration | Create temp dir, spawn livetree, perform fs ops, verify events received |
| Edge cases | Dedicated tests for symlink loops, permission errors, empty dirs |
| Performance | Benchmark with 10K-entry directory; target < 50ms full re-scan + render |

---

## Performance Considerations

- **Full re-scan vs incremental update:** We chose full re-scan for simplicity. For directories under ~10K entries, `walkdir` completes in < 5ms. Incremental updates would add significant complexity for minimal gain.
- **Debounce window:** 200ms default prevents cascading redraws during operations like `npm install` that create thousands of files. Users can tune via `--debounce`.
- **BufWriter capacity:** Default 8KB buffer is sufficient for most trees. A 1000-line tree with average 80-char lines = ~80KB, which results in ~10 flush calls — all happen within one render cycle before the final `flush()`.
- **inotify limits:** On Linux, the default max watches is 8192. Deep `node_modules` trees can exceed this. The tool should detect the error and suggest increasing the limit.

---

## Terminal State Safety

**Critical:** The tool modifies terminal state (raw mode, hidden cursor). We must **always** restore it, even on panic:

```rust
// Use a drop guard pattern
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = execute!(stdout(), cursor::Show);
    }
}

fn main() {
    let _guard = TerminalGuard;  // restored even on panic
    enable_raw_mode().unwrap();
    execute!(stdout(), cursor::Hide).unwrap();
    // ... rest of app
}
```

Without this, a crash would leave the user's terminal in raw mode (no echo, no line editing), requiring `reset` to fix.

---

## Example Output

```
📂 my-project/
├── 📂 src/
│   ├── main.rs
│   ├── tree.rs
│   ├── render.rs
│   └── watcher.rs
├── 📂 tests/
│   └── integration.rs
├── Cargo.toml
├── Cargo.lock
├── README.md
└── .gitignore

 Watching: /home/user/my-project  |  11 entries  |  Last change: 14:32:05
```

*(Tree redraws in-place when any file changes)*
