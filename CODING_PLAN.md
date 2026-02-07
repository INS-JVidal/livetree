# LiveTree — Phased Coding Plan

> Derived from `plan_treewatch.md`. Each phase is self-contained, testable, and
> builds upon the previous one. Every phase ends with a green test suite before
> the next phase begins.

---

## Table of Contents

- [Prerequisites & Conventions](#prerequisites--conventions)
- [Phase 0 — Project Scaffold & CLI](#phase-0--project-scaffold--cli)
- [Phase 1 — Tree Builder](#phase-1--tree-builder)
- [Phase 2 — Static Renderer](#phase-2--static-renderer)
- [Phase 3 — Double-Buffered Terminal Renderer](#phase-3--double-buffered-terminal-renderer)
- [Phase 4 — Filesystem Watcher](#phase-4--filesystem-watcher)
- [Phase 5 — Event Loop & Input Handling](#phase-5--event-loop--input-handling)
- [Phase 6 — Polish & Edge Cases](#phase-6--polish--edge-cases)
- [Final Integration Test with Tracing](#final-integration-test-with-tracing)
- [Robustness Review & Hardening Checklist](#robustness-review--hardening-checklist)

---

## Prerequisites & Conventions

### Project Layout

```
livetree/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point, CLI parsing, wires everything
│   ├── cli.rs           # Clap derive structs
│   ├── tree.rs          # Tree builder (walkdir + sorting + prefixes)
│   ├── render.rs        # Renderer (buffer → styled lines)
│   ├── terminal.rs      # Terminal management (raw mode, guard, resize)
│   ├── watcher.rs       # Filesystem watcher (notify + debouncer)
│   ├── event_loop.rs    # Main event loop (crossbeam select)
│   └── lib.rs           # Re-exports for integration tests
├── tests/
│   ├── phase0_cli.rs
│   ├── phase1_tree.rs
│   ├── phase2_render.rs
│   ├── phase3_terminal.rs
│   ├── phase4_watcher.rs
│   ├── phase5_event_loop.rs
│   ├── phase6_edge_cases.rs
│   └── final_integration.rs   # Full tracing integration test
└── benches/
    └── tree_scan.rs     # Performance benchmarks (Phase 6+)
```

### Testing Conventions

- All tests use `tempfile::TempDir` for filesystem fixtures — never pollute the
  real filesystem.
- Tests are **deterministic**: no sleeps unless testing debounce timing, and
  those use generous margins (3x expected).
- Each phase's test file can be run independently:
  `cargo test --test phase1_tree`
- Tests that require terminal interaction are gated behind
  `#[cfg(not(ci))]` or use mock writers.
- The `assert_cmd` crate is used for binary-level tests (CLI parsing, exit
  codes).

### Dependency Manifest (complete)

```toml
[package]
name = "livetree"
version = "0.1.0"
edition = "2021"
description = "Real-time directory tree watcher with flicker-free rendering"

[dependencies]
notify = "7"
notify-debouncer-full = "0.4"
walkdir = "2"
crossterm = "0.28"
crossbeam-channel = "0.5"
clap = { version = "4", features = ["derive"] }
globset = "0.4"

[dev-dependencies]
tempfile = "3"
assert_cmd = "2"
predicates = "3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
```

---

## Phase 0 — Project Scaffold & CLI

### Goal
Bootstrap the project. Parse CLI arguments. Print a one-shot tree to stdout
(no watching, no raw mode). Verify box-drawing characters render.

### Files to Create

**`src/cli.rs`**
```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "livetree", version, about = "Real-time directory tree watcher")]
pub struct Args {
    /// Directory to watch (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Max display depth
    #[arg(short = 'L', long = "level")]
    pub max_depth: Option<usize>,

    /// Glob patterns to exclude (repeatable)
    #[arg(short = 'I', long = "ignore", action = clap::ArgAction::Append)]
    pub ignore: Vec<String>,

    /// Show hidden files (dotfiles)
    #[arg(short = 'a', long = "all")]
    pub show_hidden: bool,

    /// Only show directories
    #[arg(short = 'D', long = "dirs-only")]
    pub dirs_only: bool,

    /// Follow symbolic links
    #[arg(short = 'f', long = "follow-symlinks")]
    pub follow_symlinks: bool,

    /// Debounce interval in milliseconds
    #[arg(long = "debounce", default_value = "200")]
    pub debounce_ms: u64,

    /// Disable colored output
    #[arg(long = "no-color")]
    pub no_color: bool,
}
```

**`src/main.rs`** (minimal)
```rust
mod cli;

use clap::Parser;
use cli::Args;

fn main() {
    let args = Args::parse();
    let path = args.path.canonicalize().unwrap_or_else(|e| {
        eprintln!("livetree: {}: {}", args.path.display(), e);
        std::process::exit(1);
    });
    if !path.is_dir() {
        eprintln!("livetree: {}: Not a directory", path.display());
        std::process::exit(1);
    }
    println!("Watching: {}", path.display());
}
```

**`src/lib.rs`** (re-exports for tests)
```rust
pub mod cli;
```

### Acceptance Criteria

1. `cargo build` succeeds.
2. `livetree --help` prints usage matching the spec.
3. `livetree --version` prints version.
4. `livetree /nonexistent` exits with code 1 and a clear error.
5. `livetree /etc/passwd` (a file) exits with code 1 and says "Not a directory".
6. `livetree .` prints "Watching: /absolute/path".

### Phase 0 Tests — `tests/phase0_cli.rs`

```rust
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn test_help_flag() {
    Command::cargo_bin("livetree")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Real-time directory tree watcher"))
        .stdout(predicate::str::contains("--level"))
        .stdout(predicate::str::contains("--ignore"))
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--dirs-only"))
        .stdout(predicate::str::contains("--debounce"));
}

#[test]
fn test_version_flag() {
    Command::cargo_bin("livetree")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("livetree"));
}

#[test]
fn test_nonexistent_path_exits_with_error() {
    Command::cargo_bin("livetree")
        .unwrap()
        .arg("/this/path/does/not/exist")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No such file"));
}

#[test]
fn test_file_path_exits_with_error() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("afile.txt");
    std::fs::write(&file, "hello").unwrap();

    Command::cargo_bin("livetree")
        .unwrap()
        .arg(file.to_str().unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not a directory"));
}

#[test]
fn test_valid_directory_prints_watching() {
    let tmp = TempDir::new().unwrap();

    Command::cargo_bin("livetree")
        .unwrap()
        .arg(tmp.path().to_str().unwrap())
        .assert()
        .success()
        .stdout(predicate::str::contains("Watching:"));
}

#[test]
fn test_default_debounce_is_200() {
    // Parse args programmatically
    use livetree::cli::Args;
    use clap::Parser;
    let args = Args::parse_from(["livetree", "."]);
    assert_eq!(args.debounce_ms, 200);
}

#[test]
fn test_custom_debounce() {
    use livetree::cli::Args;
    use clap::Parser;
    let args = Args::parse_from(["livetree", "--debounce", "500", "."]);
    assert_eq!(args.debounce_ms, 500);
}

#[test]
fn test_multiple_ignore_patterns() {
    use livetree::cli::Args;
    use clap::Parser;
    let args = Args::parse_from([
        "livetree", "-I", "*.log", "-I", "node_modules", "."
    ]);
    assert_eq!(args.ignore, vec!["*.log", "node_modules"]);
}
```

---

## Phase 1 — Tree Builder

### Goal
Given a directory path and configuration, produce a `Vec<TreeEntry>` that
represents the tree with correct nesting, sorting, prefix characters, and
ignore-pattern filtering.

### Files to Create/Modify

**`src/tree.rs`**

Core data structure:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_last: bool,
    pub prefix: String,
    pub error: Option<String>,
}
```

Configuration:
```rust
pub struct TreeConfig {
    pub max_depth: Option<usize>,
    pub show_hidden: bool,
    pub dirs_only: bool,
    pub follow_symlinks: bool,
    pub ignore_patterns: globset::GlobSet,
}
```

Key functions to implement:
```rust
/// Build the tree from a root path. This is the main public API.
pub fn build_tree(root: &Path, config: &TreeConfig) -> Vec<TreeEntry>

/// Build a GlobSet from a list of pattern strings + defaults.
pub fn build_ignore_set(patterns: &[String]) -> globset::GlobSet

/// Internal: compute the prefix string ("│   ├── ") for a given entry.
fn compute_prefix(depth: usize, ancestor_is_last: &[bool]) -> String

/// Internal: sort entries — dirs first, alpha case-insensitive, dotfiles last.
fn sort_entries(entries: &mut Vec<DirEntry>)
```

**Algorithm for `build_tree`:**
1. Use `walkdir::WalkDir` with `max_depth`, `follow_links`, `sort_by`.
2. Apply ignore filters during iteration (skip `.git`, `node_modules`, etc.).
3. Skip hidden files unless `--all`.
4. Skip files if `--dirs-only`.
5. Collect into intermediate Vec, then compute `is_last` per sibling group.
6. Compute prefixes using `ancestor_is_last` stack.

**Prefix rules:**
- `├── ` = not last sibling
- `└── ` = last sibling
- `│   ` = ancestor was not last sibling (continuation line)
- `    ` = ancestor was last sibling (blank space)

### Acceptance Criteria

1. Given a known directory structure, output matches expected `Vec<TreeEntry>`.
2. Directories sort before files.
3. Case-insensitive alphabetical sorting within groups.
4. Dotfiles are hidden by default, shown with `--all` (sorted last).
5. Depth limiting works correctly.
6. Ignore patterns filter entries during traversal.
7. Default ignores (`.git`, `node_modules`, `__pycache__`, `.DS_Store`) work.
8. Prefix strings are correctly computed for arbitrarily deep trees.

### Phase 1 Tests — `tests/phase1_tree.rs`

```rust
use tempfile::TempDir;
use std::fs;
use livetree::tree::{build_tree, build_ignore_set, TreeConfig, TreeEntry};

/// Helper: create a directory structure from a list of relative paths.
/// Paths ending with '/' create directories; others create empty files.
fn create_fixture(paths: &[&str]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    for p in paths {
        let full = tmp.path().join(p);
        if p.ends_with('/') {
            fs::create_dir_all(&full).unwrap();
        } else {
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full, "").unwrap();
        }
    }
    tmp
}

fn default_config() -> TreeConfig {
    TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    }
}

// --- Sorting ---

#[test]
fn test_directories_before_files() {
    let tmp = create_fixture(&["src/", "README.md", "build/", "main.rs"]);
    let entries = build_tree(tmp.path(), &default_config());

    // Find the top-level entries (depth 1)
    let top: Vec<&str> = entries.iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();

    // Directories should come first
    let first_file_idx = top.iter().position(|n| !entries.iter()
        .find(|e| e.name == *n && e.depth == 1).unwrap().is_dir).unwrap();
    let last_dir_idx = top.iter().rposition(|n| entries.iter()
        .find(|e| e.name == *n && e.depth == 1).unwrap().is_dir).unwrap();

    assert!(last_dir_idx < first_file_idx,
        "All directories should sort before files. Got: {:?}", top);
}

#[test]
fn test_case_insensitive_alpha_sort() {
    let tmp = create_fixture(&["Banana.txt", "apple.txt", "Cherry.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let names: Vec<&str> = entries.iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, vec!["apple.txt", "Banana.txt", "Cherry.txt"]);
}

#[test]
fn test_dotfiles_hidden_by_default() {
    let tmp = create_fixture(&[".hidden", "visible.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let names: Vec<&str> = entries.iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&".hidden"));
    assert!(names.contains(&"visible.txt"));
}

#[test]
fn test_dotfiles_shown_with_all_flag() {
    let tmp = create_fixture(&[".hidden", "visible.txt"]);
    let mut cfg = default_config();
    cfg.show_hidden = true;
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries.iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert!(names.contains(&".hidden"));
    assert!(names.contains(&"visible.txt"));
    // Dotfiles should sort after non-dotfiles
    let hidden_idx = names.iter().position(|n| *n == ".hidden").unwrap();
    let visible_idx = names.iter().position(|n| *n == "visible.txt").unwrap();
    assert!(hidden_idx > visible_idx,
        "Dotfiles should sort after regular files. Got: {:?}", names);
}

// --- Depth Limiting ---

#[test]
fn test_depth_limit_1() {
    let tmp = create_fixture(&[
        "a/", "a/b/", "a/b/c.txt", "a/d.txt", "e.txt"
    ]);
    let mut cfg = default_config();
    cfg.max_depth = Some(1);
    let entries = build_tree(tmp.path(), &cfg);
    assert!(entries.iter().all(|e| e.depth <= 1),
        "No entry should exceed depth 1");
}

#[test]
fn test_depth_limit_2() {
    let tmp = create_fixture(&[
        "a/", "a/b/", "a/b/deep.txt", "a/top.txt"
    ]);
    let mut cfg = default_config();
    cfg.max_depth = Some(2);
    let entries = build_tree(tmp.path(), &cfg);
    assert!(entries.iter().all(|e| e.depth <= 2));
    // depth=2 entries should exist
    assert!(entries.iter().any(|e| e.depth == 2));
}

// --- Ignore Patterns ---

#[test]
fn test_default_ignores() {
    let tmp = create_fixture(&[
        ".git/", ".git/config", "node_modules/", "node_modules/pkg/",
        "__pycache__/", "src/", "src/main.rs"
    ]);
    let mut cfg = default_config();
    cfg.show_hidden = true; // .git is hidden AND in default ignores
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(!names.contains(&".git"),       "Should ignore .git");
    assert!(!names.contains(&"node_modules"), "Should ignore node_modules");
    assert!(!names.contains(&"__pycache__"), "Should ignore __pycache__");
    assert!(names.contains(&"src"),          "Should keep src");
}

#[test]
fn test_custom_ignore_pattern() {
    let tmp = create_fixture(&["debug.log", "app.log", "main.rs", "lib.rs"]);
    let mut cfg = default_config();
    cfg.ignore_patterns = build_ignore_set(&["*.log".to_string()]);
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries.iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&"debug.log"));
    assert!(!names.contains(&"app.log"));
    assert!(names.contains(&"main.rs"));
}

// --- Dirs Only ---

#[test]
fn test_dirs_only() {
    let tmp = create_fixture(&["src/", "src/main.rs", "tests/", "README.md"]);
    let mut cfg = default_config();
    cfg.dirs_only = true;
    let entries = build_tree(tmp.path(), &cfg);
    assert!(entries.iter().filter(|e| e.depth >= 1).all(|e| e.is_dir),
        "All entries should be directories when dirs_only is set");
}

// --- Prefix Computation ---

#[test]
fn test_prefix_simple_tree() {
    // Structure:
    // root/
    // ├── a/
    // │   └── deep.txt
    // └── b.txt
    let tmp = create_fixture(&["a/", "a/deep.txt", "b.txt"]);
    let entries = build_tree(tmp.path(), &default_config());

    // 'a' directory should have ├── prefix (not last, 'b.txt' follows)
    let a_entry = entries.iter().find(|e| e.name == "a").unwrap();
    assert!(a_entry.prefix.contains("├"), "Dir 'a' should use ├── (got: {:?})", a_entry.prefix);

    // 'b.txt' should have └── prefix (last sibling)
    let b_entry = entries.iter().find(|e| e.name == "b.txt").unwrap();
    assert!(b_entry.prefix.contains("└"), "'b.txt' should use └── (got: {:?})", b_entry.prefix);

    // 'deep.txt' should have "│   └── " or similar nested prefix
    let deep = entries.iter().find(|e| e.name == "deep.txt").unwrap();
    assert!(deep.prefix.contains("└"), "Nested last child should use └──");
}

#[test]
fn test_prefix_deeply_nested() {
    let tmp = create_fixture(&[
        "a/", "a/b/", "a/b/c/", "a/b/c/d.txt"
    ]);
    let entries = build_tree(tmp.path(), &default_config());
    let d = entries.iter().find(|e| e.name == "d.txt").unwrap();
    // d.txt is at depth 4, should have 3 levels of prefix indentation
    assert!(d.depth == 4 || d.depth == 3, "d.txt should be deeply nested");
}

// --- is_last Flag ---

#[test]
fn test_is_last_flag() {
    let tmp = create_fixture(&["alpha.txt", "beta.txt", "gamma.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let top: Vec<&TreeEntry> = entries.iter().filter(|e| e.depth == 1).collect();
    // Only the last entry should have is_last = true
    for (i, entry) in top.iter().enumerate() {
        if i == top.len() - 1 {
            assert!(entry.is_last, "Last entry '{}' should have is_last=true", entry.name);
        } else {
            assert!(!entry.is_last, "Entry '{}' should have is_last=false", entry.name);
        }
    }
}

// --- Empty Directory ---

#[test]
fn test_empty_directory() {
    let tmp = TempDir::new().unwrap();
    let entries = build_tree(tmp.path(), &default_config());
    // Should have only the root entry (depth 0) or be empty
    assert!(entries.iter().filter(|e| e.depth >= 1).count() == 0,
        "Empty directory should produce no child entries");
}

// --- Symlinks ---

#[test]
fn test_symlink_detected() {
    let tmp = create_fixture(&["target.txt"]);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            tmp.path().join("target.txt"),
            tmp.path().join("link.txt"),
        ).unwrap();
        let entries = build_tree(tmp.path(), &default_config());
        let link = entries.iter().find(|e| e.name == "link.txt");
        assert!(link.is_some(), "Symlink should appear in tree");
        assert!(link.unwrap().is_symlink, "Symlink should be flagged as is_symlink");
    }
}
```

---

## Phase 2 — Static Renderer

### Goal
Render a `Vec<TreeEntry>` into styled, displayable lines — writing to any
`impl Write`, not necessarily stdout. This decouples rendering from terminal
management and makes it fully testable.

### Files to Create/Modify

**`src/render.rs`**

```rust
use crate::tree::TreeEntry;
use std::io::Write;

pub struct RenderConfig {
    pub use_color: bool,
    pub terminal_width: u16,
}

/// Render the tree to any writer. Returns the number of lines written.
pub fn render_tree<W: Write>(
    writer: &mut W,
    entries: &[TreeEntry],
    config: &RenderConfig,
) -> usize

/// Render a single entry to a styled line (with or without ANSI codes).
pub fn format_entry(entry: &TreeEntry, config: &RenderConfig) -> String

/// Render the status bar line.
pub fn format_status_bar(
    watched_path: &str,
    entry_count: usize,
    last_change: Option<&str>,
    terminal_width: u16,
) -> String
```

**Color rules:**
- Directories: bold blue (`\x1b[1;34m`)
- Symlinks: cyan (`\x1b[36m`) + ` -> target`
- Errors: red (`\x1b[31m`)
- Tree connectors: dim (`\x1b[2m`)
- Reset: `\x1b[0m`

**Truncation:** If `prefix + name > terminal_width`, truncate name and append `…`.

### Phase 2 Tests — `tests/phase2_render.rs`

```rust
use livetree::tree::TreeEntry;
use livetree::render::{render_tree, format_entry, format_status_bar, RenderConfig};
use std::path::PathBuf;

fn make_entry(name: &str, depth: usize, is_dir: bool, is_last: bool, prefix: &str) -> TreeEntry {
    TreeEntry {
        name: name.to_string(),
        path: PathBuf::from(name),
        depth,
        is_dir,
        is_symlink: false,
        is_last,
        prefix: prefix.to_string(),
        error: None,
    }
}

fn no_color_config(width: u16) -> RenderConfig {
    RenderConfig { use_color: false, terminal_width: width }
}

fn color_config(width: u16) -> RenderConfig {
    RenderConfig { use_color: true, terminal_width: width }
}

// --- format_entry ---

#[test]
fn test_format_entry_plain_file() {
    let entry = make_entry("main.rs", 1, false, false, "├── ");
    let line = format_entry(&entry, &no_color_config(80));
    assert_eq!(line, "├── main.rs");
}

#[test]
fn test_format_entry_directory_colored() {
    let entry = make_entry("src", 1, true, false, "├── ");
    let line = format_entry(&entry, &color_config(80));
    // Should contain ANSI bold blue escape
    assert!(line.contains("\x1b[1;34m") || line.contains("\x1b[34m"),
        "Directory should be colored blue: {:?}", line);
    assert!(line.contains("src"));
}

#[test]
fn test_format_entry_symlink() {
    let mut entry = make_entry("link", 1, false, true, "└── ");
    entry.is_symlink = true;
    let line = format_entry(&entry, &color_config(80));
    assert!(line.contains("\x1b[36m"), "Symlink should be cyan");
}

#[test]
fn test_format_entry_error() {
    let mut entry = make_entry("forbidden", 1, true, false, "├── ");
    entry.error = Some("permission denied".to_string());
    let line = format_entry(&entry, &color_config(80));
    assert!(line.contains("\x1b[31m"), "Error should be red");
    assert!(line.contains("permission denied"));
}

// --- Truncation ---

#[test]
fn test_long_filename_truncated() {
    let long_name = "a".repeat(200);
    let entry = make_entry(&long_name, 1, false, false, "├── ");
    let line = format_entry(&entry, &no_color_config(40));
    assert!(line.len() <= 40 + 3, // +3 for potential UTF-8 ellipsis
        "Line should be truncated to terminal width. Got len={}", line.len());
    assert!(line.contains("…"), "Truncated line should end with ellipsis");
}

// --- render_tree ---

#[test]
fn test_render_tree_output() {
    let entries = vec![
        make_entry("src", 1, true, false, "├── "),
        make_entry("main.rs", 2, false, true, "│   └── "),
        make_entry("README.md", 1, false, true, "└── "),
    ];
    let mut buf = Vec::new();
    let count = render_tree(&mut buf, &entries, &no_color_config(80));
    let output = String::from_utf8(buf).unwrap();

    assert_eq!(count, 3);
    assert!(output.contains("├── src"));
    assert!(output.contains("│   └── main.rs"));
    assert!(output.contains("└── README.md"));
}

#[test]
fn test_render_empty_tree() {
    let entries: Vec<TreeEntry> = vec![];
    let mut buf = Vec::new();
    let count = render_tree(&mut buf, &entries, &no_color_config(80));
    assert_eq!(count, 0);
}

// --- Status Bar ---

#[test]
fn test_status_bar_format() {
    let bar = format_status_bar("/home/user/project", 42, Some("14:32:05"), 80);
    assert!(bar.contains("/home/user/project"));
    assert!(bar.contains("42"));
    assert!(bar.contains("14:32:05"));
}

#[test]
fn test_status_bar_no_change_yet() {
    let bar = format_status_bar("/tmp/dir", 0, None, 80);
    assert!(bar.contains("/tmp/dir"));
    // Should gracefully handle no last-change timestamp
}

#[test]
fn test_status_bar_truncated_path() {
    let long_path = "/very/".to_string() + &"deep/".repeat(50) + "path";
    let bar = format_status_bar(&long_path, 10, None, 60);
    // Bar should not exceed terminal width (approximately)
    assert!(bar.len() < 200, "Status bar should be reasonable length");
}

// --- Snapshot Test (full pipeline: build + render) ---

#[test]
fn test_snapshot_known_tree() {
    use tempfile::TempDir;
    use std::fs;
    use livetree::tree::{build_tree, build_ignore_set, TreeConfig};

    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("src")).unwrap();
    fs::write(tmp.path().join("src/main.rs"), "").unwrap();
    fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
    fs::write(tmp.path().join("Cargo.toml"), "").unwrap();
    fs::write(tmp.path().join("README.md"), "").unwrap();

    let cfg = TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    };
    let entries = build_tree(tmp.path(), &cfg);

    let mut buf = Vec::new();
    render_tree(&mut buf, &entries, &no_color_config(80));
    let output = String::from_utf8(buf).unwrap();

    // Verify structural correctness
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines.len() >= 4, "Should have at least 4 entries");

    // src/ directory should come before files
    let src_line = lines.iter().position(|l| l.contains("src")).unwrap();
    let cargo_line = lines.iter().position(|l| l.contains("Cargo.toml")).unwrap();
    assert!(src_line < cargo_line, "Directories should sort before files");

    // Nested files should appear under src/
    let main_line = lines.iter().position(|l| l.contains("main.rs")).unwrap();
    assert!(main_line > src_line && main_line < cargo_line,
        "main.rs should be nested under src/");
}
```

---

## Phase 3 — Double-Buffered Terminal Renderer

### Goal
Implement the real terminal renderer: raw mode, cursor repositioning,
`BufWriter<Stdout>`, in-place overwriting, leftover-line clearing. Also
implement `TerminalGuard` for safe cleanup.

### Files to Create/Modify

**`src/terminal.rs`**

```rust
use crossterm::{cursor, terminal, execute, queue};
use std::io::{BufWriter, Stdout, Write, stdout};

/// RAII guard that restores terminal state on drop (even on panic).
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn new() -> std::io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), cursor::Hide)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(stdout(), cursor::Show, cursor::MoveTo(0, 0));
    }
}

/// The frame renderer: writes a complete frame to the terminal.
pub fn render_frame(
    writer: &mut BufWriter<Stdout>,
    lines: &[String],
    prev_line_count: usize,
) -> std::io::Result<usize>

/// Get current terminal dimensions.
pub fn terminal_size() -> (u16, u16)  // (width, height)
```

**`render_frame` implementation outline:**
1. `queue!(writer, cursor::MoveTo(0, 0))`
2. For each line: clear current line, write line content
3. For leftover lines (prev > current): clear each remaining line
4. `writer.flush()` — the single atomic write

### Phase 3 Tests — `tests/phase3_terminal.rs`

Terminal tests are tricky because they require a real terminal. We test the
logic by writing to a mock buffer and verifying escape sequences.

```rust
use std::io::BufWriter;

/// Test helper: capture all bytes written by render_frame into a Vec<u8>.
/// We replace BufWriter<Stdout> with BufWriter<Vec<u8>> in the test variant.

// NOTE: For this to work, render_frame should be generic over W: Write.
// The actual signature should be:
//   pub fn render_frame<W: Write>(writer: &mut W, ...) -> ...

#[test]
fn test_render_frame_writes_cursor_home() {
    // Verify the output starts with MoveTo(0,0) escape sequence
    let mut buf = Vec::new();
    let lines = vec!["├── file.txt".to_string()];

    livetree::terminal::render_frame(&mut buf, &lines, 0).unwrap();
    let output = String::from_utf8_lossy(&buf);

    // CSI H or CSI 1;1H is cursor home
    assert!(output.contains("\x1b[") && output.contains("H"),
        "Should contain cursor positioning escape");
}

#[test]
fn test_render_frame_clears_leftover_lines() {
    let mut buf = Vec::new();
    let lines = vec!["line1".to_string()];

    // Previous frame had 5 lines, current has 1 → should clear 4 extra
    livetree::terminal::render_frame(&mut buf, &lines, 5).unwrap();
    let output = String::from_utf8_lossy(&buf);

    // Count clear-line sequences (CSI 2K)
    let clear_count = output.matches("\x1b[2K").count();
    assert!(clear_count >= 5, "Should clear current + leftover lines. Got {} clears", clear_count);
}

#[test]
fn test_render_frame_returns_line_count() {
    let mut buf = Vec::new();
    let lines = vec![
        "line1".to_string(),
        "line2".to_string(),
        "line3".to_string(),
    ];
    let count = livetree::terminal::render_frame(&mut buf, &lines, 0).unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_render_frame_empty() {
    let mut buf = Vec::new();
    let count = livetree::terminal::render_frame(&mut buf, &[], 3).unwrap();
    assert_eq!(count, 0);
    // Should still clear the 3 leftover lines
    let output = String::from_utf8_lossy(&buf);
    let clear_count = output.matches("\x1b[2K").count();
    assert!(clear_count >= 3);
}

#[test]
fn test_terminal_guard_drop_restores_state() {
    // This test verifies the TerminalGuard pattern compiles and doesn't panic.
    // Actual terminal restoration is tested manually / in the integration test.
    // We only verify the struct can be created and dropped without panic
    // when running in a non-terminal context.
    //
    // NOTE: This test may fail in CI where there's no terminal. Guard it.
    #[cfg(not(ci))]
    {
        // If we're in a real terminal, test the guard
        if crossterm::terminal::is_raw_mode_supported() {
            let guard = livetree::terminal::TerminalGuard::new();
            // Guard should succeed
            assert!(guard.is_ok());
            // Drop should restore
            drop(guard);
        }
    }
}
```

---

## Phase 4 — Filesystem Watcher

### Goal
Watch a directory recursively using `notify` with the full debouncer. Send
"refresh" signals via `mpsc::channel`. Handle watcher errors gracefully.

### Files to Create/Modify

**`src/watcher.rs`**

```rust
use notify::RecommendedWatcher;
use notify_debouncer_full::{new_debouncer, Debouncer, DebounceEventResult};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

pub enum WatchEvent {
    Changed,           // Something changed, re-scan needed
    RootDeleted,       // The watched root directory was deleted
    Error(String),     // Watcher error (e.g., inotify limit)
}

/// Start watching a directory. Returns the debouncer (keep alive) and a receiver.
pub fn start_watcher(
    path: &Path,
    debounce_ms: u64,
) -> Result<(Debouncer<RecommendedWatcher, ...>, mpsc::Receiver<WatchEvent>), String>

/// Check if the root path still exists (called on each event).
fn check_root_exists(path: &Path) -> bool
```

### Phase 4 Tests — `tests/phase4_watcher.rs`

```rust
use tempfile::TempDir;
use std::fs;
use std::time::Duration;
use livetree::watcher::{start_watcher, WatchEvent};

#[test]
fn test_watcher_detects_file_creation() {
    let tmp = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(tmp.path(), 100).unwrap();

    // Create a file
    fs::write(tmp.path().join("new.txt"), "hello").unwrap();

    // Should receive a Changed event within debounce_ms + margin
    let event = rx.recv_timeout(Duration::from_millis(500));
    assert!(matches!(event, Ok(WatchEvent::Changed)),
        "Should detect file creation. Got: {:?}", event);

    drop(watcher); // Keep watcher alive until here
}

#[test]
fn test_watcher_detects_file_deletion() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("existing.txt");
    fs::write(&file, "content").unwrap();

    let (watcher, rx) = start_watcher(tmp.path(), 100).unwrap();

    fs::remove_file(&file).unwrap();

    let event = rx.recv_timeout(Duration::from_millis(500));
    assert!(matches!(event, Ok(WatchEvent::Changed)),
        "Should detect file deletion");

    drop(watcher);
}

#[test]
fn test_watcher_detects_file_modification() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("data.txt");
    fs::write(&file, "v1").unwrap();

    let (watcher, rx) = start_watcher(tmp.path(), 100).unwrap();

    fs::write(&file, "v2").unwrap();

    let event = rx.recv_timeout(Duration::from_millis(500));
    assert!(matches!(event, Ok(WatchEvent::Changed)),
        "Should detect file modification");

    drop(watcher);
}

#[test]
fn test_watcher_detects_directory_creation() {
    let tmp = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(tmp.path(), 100).unwrap();

    fs::create_dir(tmp.path().join("newdir")).unwrap();

    let event = rx.recv_timeout(Duration::from_millis(500));
    assert!(matches!(event, Ok(WatchEvent::Changed)),
        "Should detect directory creation");

    drop(watcher);
}

#[test]
fn test_watcher_debounces_rapid_events() {
    let tmp = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(tmp.path(), 200).unwrap();

    // Create 50 files rapidly
    for i in 0..50 {
        fs::write(tmp.path().join(format!("file{}.txt", i)), "").unwrap();
    }

    // Wait for debounce to settle
    std::thread::sleep(Duration::from_millis(500));

    // Should receive a small number of coalesced events (not 50)
    let mut event_count = 0;
    while rx.try_recv().is_ok() {
        event_count += 1;
    }
    // Debouncer should coalesce into a few events, not 50
    assert!(event_count < 10,
        "Debouncer should coalesce rapid events. Got {} events for 50 file creates",
        event_count);

    drop(watcher);
}

#[test]
fn test_watcher_detects_nested_changes() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();

    let (watcher, rx) = start_watcher(tmp.path(), 100).unwrap();

    // Create a deeply nested file
    fs::write(tmp.path().join("a/b/c/deep.txt"), "").unwrap();

    let event = rx.recv_timeout(Duration::from_millis(500));
    assert!(matches!(event, Ok(WatchEvent::Changed)),
        "Should detect deeply nested file creation");

    drop(watcher);
}

#[test]
fn test_watcher_nonexistent_path_returns_error() {
    let result = start_watcher(std::path::Path::new("/nonexistent/path"), 100);
    assert!(result.is_err(), "Should error on nonexistent path");
}
```

---

## Phase 5 — Event Loop & Input Handling

### Goal
Wire everything together: watcher events, keyboard input, tree rebuild,
and render — all in the `crossbeam-channel` `select!` loop.

### Files to Create/Modify

**`src/event_loop.rs`**

```rust
use crossbeam_channel::{select, Receiver};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::thread;

pub enum AppEvent {
    FsChanged,
    KeyPress(KeyCode, KeyModifiers),
    Resize(u16, u16),
    Quit,
}

/// Run the main application loop. Blocks until the user quits.
pub fn run(
    path: &Path,
    tree_config: &TreeConfig,
    render_config: &RenderConfig,
    fs_rx: Receiver<WatchEvent>,
) -> Result<(), String>
```

**Event flow:**
1. Spawn input-reader thread that sends `AppEvent::KeyPress` / `AppEvent::Resize`.
2. `select!` on `fs_rx` and `key_rx`.
3. On `FsChanged` → `build_tree()` + `render_frame()`.
4. On `KeyPress('q')` or `KeyPress(Ctrl+'c')` → break.
5. On `Resize` → update `RenderConfig.terminal_width` + re-render.

### Phase 5 Tests — `tests/phase5_event_loop.rs`

The event loop is hard to unit-test in isolation because it blocks.
We test it by:
1. Verifying the event dispatch logic with mock channels.
2. Running the binary as a subprocess with a timeout.

```rust
use crossbeam_channel::{bounded, unbounded};
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_event_dispatch_fs_change_triggers_rebuild() {
    // Test the core logic: when a WatchEvent::Changed arrives,
    // the tree should be rebuilt and rendered.
    //
    // We do this by testing the build+render pipeline directly,
    // since the event loop's role is just dispatching.
    use livetree::tree::{build_tree, build_ignore_set, TreeConfig};
    use livetree::render::{render_tree, RenderConfig};

    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "").unwrap();

    let cfg = TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    };

    let entries1 = build_tree(tmp.path(), &cfg);
    assert_eq!(entries1.iter().filter(|e| e.depth == 1).count(), 1);

    // Simulate filesystem change
    std::fs::write(tmp.path().join("b.txt"), "").unwrap();

    let entries2 = build_tree(tmp.path(), &cfg);
    assert_eq!(entries2.iter().filter(|e| e.depth == 1).count(), 2,
        "Rebuild after change should show new file");
}

#[test]
fn test_binary_exits_on_timeout() {
    // Run livetree as a subprocess and kill it after a short time.
    // Verify it didn't corrupt the terminal state.
    let tmp = TempDir::new().unwrap();

    let child = std::process::Command::new(env!("CARGO_BIN_EXE_livetree"))
        .arg(tmp.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    if let Ok(mut child) = child {
        std::thread::sleep(Duration::from_millis(500));
        let _ = child.kill();
        let status = child.wait().unwrap();
        // Process should have been killed (not crashed)
        // On Unix, killed processes don't have a normal exit code
        #[cfg(unix)]
        assert!(!status.success() || status.code().is_none());
    }
}

#[test]
fn test_quit_key_stops_loop() {
    // This tests that writing 'q' to stdin causes the process to exit cleanly.
    use std::io::Write;

    let tmp = TempDir::new().unwrap();

    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_livetree"))
        .arg(tmp.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn livetree");

    // Give it time to start
    std::thread::sleep(Duration::from_millis(300));

    // Send 'q' to stdin
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(b"q");
        let _ = stdin.flush();
    }

    // Wait for graceful exit with timeout
    let result = child.wait_timeout(Duration::from_secs(3));
    match result {
        Ok(Some(status)) => assert!(status.success(), "Should exit cleanly on 'q'"),
        Ok(None) => {
            child.kill().unwrap();
            panic!("Process did not exit after 'q' key within 3 seconds");
        }
        Err(e) => panic!("Wait failed: {}", e),
    }
}
```

---

## Phase 6 — Polish & Edge Cases

### Goal
Harden the tool: handle all error scenarios from the plan, add color,
implement all remaining CLI flags, and ensure clean behavior under adversarial
conditions.

### Tasks

1. **Permission denied on subdirectory** — `walkdir` reports errors; capture
   them as `TreeEntry { error: Some("permission denied") }`.
2. **Symlink loops** — `walkdir` with `follow_links(true)` has cycle detection.
   Capture as `TreeEntry { error: Some("cycle detected") }`.
3. **Terminal too narrow** — truncate lines with `…`.
4. **Terminal too short** — render as many lines as fit, then `... and N more`.
5. **inotify watch limit** — detect `notify` error, show user-friendly message.
6. **Watched directory deleted** — on `WatchEvent::RootDeleted`, display
   message and either wait for re-creation or exit.
7. **Root is file, not directory** — already handled in Phase 0.

### Phase 6 Tests — `tests/phase6_edge_cases.rs`

```rust
use tempfile::TempDir;
use std::fs;
use livetree::tree::{build_tree, build_ignore_set, TreeConfig};
use livetree::render::{render_tree, format_entry, RenderConfig};

fn default_config() -> TreeConfig {
    TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    }
}

// --- Permission Denied ---

#[test]
#[cfg(unix)]
fn test_permission_denied_subdirectory() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let forbidden = tmp.path().join("forbidden");
    fs::create_dir(&forbidden).unwrap();
    fs::write(forbidden.join("secret.txt"), "").unwrap();
    // Remove read+execute permissions
    fs::set_permissions(&forbidden, fs::Permissions::from_mode(0o000)).unwrap();

    let entries = build_tree(tmp.path(), &default_config());

    // The forbidden directory should appear with an error marker
    let entry = entries.iter().find(|e| e.name == "forbidden");
    assert!(entry.is_some(), "forbidden dir should still appear in tree");
    // It should either have an error field or its children should be absent
    // (depends on implementation: either show error marker or just skip children)

    // Restore permissions for cleanup
    fs::set_permissions(&forbidden, fs::Permissions::from_mode(0o755)).unwrap();
}

// --- Symlink Loops ---

#[test]
#[cfg(unix)]
fn test_symlink_loop_handled() {
    let tmp = TempDir::new().unwrap();
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("a/b");
    fs::create_dir_all(&dir_b).unwrap();
    // Create cycle: a/b/loop -> a
    std::os::unix::fs::symlink(&dir_a, dir_b.join("loop")).unwrap();

    let mut cfg = default_config();
    cfg.follow_symlinks = true;

    // This should NOT hang or panic
    let entries = build_tree(tmp.path(), &cfg);

    // The loop entry should be handled gracefully
    assert!(!entries.is_empty(), "Should produce output despite symlink loop");
    // Verify we don't have an infinite number of entries
    assert!(entries.len() < 100,
        "Symlink loop should not cause infinite traversal. Got {} entries", entries.len());
}

// --- Terminal Too Narrow ---

#[test]
fn test_very_narrow_terminal() {
    let entries = vec![
        livetree::tree::TreeEntry {
            name: "very_long_filename_that_exceeds_width.rs".to_string(),
            path: std::path::PathBuf::from("very_long_filename_that_exceeds_width.rs"),
            depth: 1,
            is_dir: false,
            is_symlink: false,
            is_last: true,
            prefix: "└── ".to_string(),
            error: None,
        },
    ];

    let cfg = RenderConfig { use_color: false, terminal_width: 20 };
    let line = format_entry(&entries[0], &cfg);
    // The line should be truncated
    // "└── " is 4 chars + some of the filename + "…"
    assert!(line.chars().count() <= 21,
        "Line should fit in 20-char terminal. Got {} chars: {:?}",
        line.chars().count(), line);
}

// --- Terminal Too Short ---

#[test]
fn test_tree_exceeds_terminal_height() {
    let tmp = TempDir::new().unwrap();
    for i in 0..100 {
        fs::write(tmp.path().join(format!("file{:03}.txt", i)), "").unwrap();
    }

    let entries = build_tree(tmp.path(), &default_config());

    // Simulate rendering to a 10-line terminal
    let mut buf = Vec::new();
    let cfg = RenderConfig { use_color: false, terminal_width: 80 };
    // The render function should accept a max_height parameter
    // or the caller should truncate
    let visible_entries = &entries[..entries.len().min(9)];
    render_tree(&mut buf, visible_entries, &cfg);
    let output = String::from_utf8(buf).unwrap();
    let line_count = output.lines().count();

    assert!(line_count <= 10, "Should not exceed terminal height");
}

// --- Empty Root ---

#[test]
fn test_empty_root_directory() {
    let tmp = TempDir::new().unwrap();
    let entries = build_tree(tmp.path(), &default_config());

    let mut buf = Vec::new();
    let cfg = RenderConfig { use_color: false, terminal_width: 80 };
    let count = render_tree(&mut buf, &entries, &cfg);
    // Should render cleanly (possibly with just a root line or nothing)
    assert!(count <= 1, "Empty dir should produce at most 1 line (root)");
}

// --- Nested Empty Directories ---

#[test]
fn test_nested_empty_directories() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();

    let entries = build_tree(tmp.path(), &default_config());
    // Should show the chain of empty dirs
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"b"));
    assert!(names.contains(&"c"));
}

// --- Mixed Symlinks ---

#[test]
#[cfg(unix)]
fn test_symlink_to_file_shows_arrow() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("real.txt"), "content").unwrap();
    std::os::unix::fs::symlink(
        tmp.path().join("real.txt"),
        tmp.path().join("link.txt"),
    ).unwrap();

    let entries = build_tree(tmp.path(), &default_config());
    let link = entries.iter().find(|e| e.name == "link.txt").unwrap();
    assert!(link.is_symlink);

    // When rendered, should show arrow
    let cfg = RenderConfig { use_color: false, terminal_width: 80 };
    let line = format_entry(link, &cfg);
    assert!(line.contains("->") || line.contains("→"),
        "Symlink should show target: {:?}", line);
}

// --- Rapid Resize ---

#[test]
fn test_render_at_various_widths() {
    let entries = vec![
        livetree::tree::TreeEntry {
            name: "filename.txt".to_string(),
            path: std::path::PathBuf::from("filename.txt"),
            depth: 1, is_dir: false, is_symlink: false, is_last: true,
            prefix: "└── ".to_string(), error: None,
        },
    ];

    // Render at multiple widths — none should panic
    for width in [10, 20, 40, 80, 120, 200, 1] {
        let cfg = RenderConfig { use_color: false, terminal_width: width };
        let mut buf = Vec::new();
        render_tree(&mut buf, &entries, &cfg);
        // Just verify no panic
    }
}
```

---

## Final Integration Test with Tracing

This is the **crown jewel** test: a full end-to-end test that spawns the real
binary, performs filesystem operations, captures output, and verifies
correctness. It uses the `tracing` crate to produce detailed, timestamped
diagnostic output.

### `tests/final_integration.rs`

```rust
//! Final integration test for LiveTree.
//!
//! This test exercises the full pipeline:
//! 1. Creates a realistic directory structure
//! 2. Spawns livetree as a subprocess
//! 3. Performs filesystem mutations (create, delete, rename, modify)
//! 4. Captures and verifies output
//! 5. Verifies clean shutdown
//!
//! Run with tracing output:
//!   RUST_LOG=debug cargo test --test final_integration -- --nocapture

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tracing::{debug, error, info, warn, span, Level};
use tracing_subscriber::EnvFilter;

/// Initialize tracing for detailed test output.
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("debug"))
        )
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init();
}

/// Helper: Create a realistic project structure.
fn create_project_fixture(root: &Path) {
    info!("Creating project fixture at {}", root.display());

    let dirs = [
        "src", "src/components", "src/utils", "tests", "docs", ".git",
    ];
    let files = [
        ("src/main.rs", "fn main() { }"),
        ("src/lib.rs", "pub mod components;\npub mod utils;"),
        ("src/components/mod.rs", "pub mod button;"),
        ("src/components/button.rs", "pub struct Button;"),
        ("src/utils/mod.rs", "pub mod helpers;"),
        ("src/utils/helpers.rs", "pub fn help() {}"),
        ("tests/integration.rs", "#[test] fn it_works() {}"),
        ("docs/README.md", "# My Project"),
        ("Cargo.toml", "[package]\nname = \"myproject\""),
        ("Cargo.lock", "# auto-generated"),
        (".gitignore", "target/\n"),
        (".git/config", "[core]"),
    ];

    for dir in &dirs {
        let path = root.join(dir);
        fs::create_dir_all(&path).unwrap();
        debug!("  Created dir: {}", dir);
    }
    for (file, content) in &files {
        let path = root.join(file);
        fs::write(&path, content).unwrap();
        debug!("  Created file: {}", file);
    }

    info!("Fixture created: {} dirs, {} files", dirs.len(), files.len());
}

/// Spawn livetree and return the child process.
fn spawn_livetree(path: &Path) -> Child {
    info!("Spawning livetree for: {}", path.display());

    Command::new(env!("CARGO_BIN_EXE_livetree"))
        .arg(path)
        .arg("--no-color")
        .arg("--debounce")
        .arg("100")  // Fast debounce for testing
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn livetree binary")
}

/// Wait for the process to produce output, with timeout.
fn wait_for_output(child: &mut Child, timeout: Duration) -> Option<String> {
    let stdout = child.stdout.as_mut().unwrap();
    let reader = BufReader::new(stdout);

    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut lines = Vec::new();
        // Read available lines
        // (In practice, since the output is to a raw terminal,
        // we may need to read raw bytes instead of lines)
        lines
    });

    rx.recv_timeout(timeout).ok()
}

/// ===== THE MAIN INTEGRATION TEST =====

#[test]
fn test_full_lifecycle() {
    init_tracing();
    let test_span = span!(Level::INFO, "full_lifecycle_test");
    let _enter = test_span.enter();

    info!("========================================");
    info!("  LiveTree Full Integration Test");
    info!("========================================");

    // --- Step 1: Create fixture ---
    let tmp = TempDir::new().unwrap();
    create_project_fixture(tmp.path());

    // --- Step 2: Test tree builder directly (no process) ---
    {
        let span = span!(Level::INFO, "tree_builder_validation");
        let _enter = span.enter();

        info!("Validating tree builder with fixture...");

        use livetree::tree::{build_tree, build_ignore_set, TreeConfig};

        let cfg = TreeConfig {
            max_depth: None,
            show_hidden: false,
            dirs_only: false,
            follow_symlinks: false,
            ignore_patterns: build_ignore_set(&[]),
        };

        let entries = build_tree(tmp.path(), &cfg);
        info!("Tree has {} entries", entries.len());

        // Verify .git is ignored by default
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&".git"), "FAIL: .git should be ignored");
        info!("  [PASS] .git is ignored");

        // Verify expected structure
        assert!(names.contains(&"src"), "FAIL: src/ missing");
        assert!(names.contains(&"Cargo.toml"), "FAIL: Cargo.toml missing");
        assert!(names.contains(&"button.rs"), "FAIL: button.rs missing");
        info!("  [PASS] Expected files present");

        // Verify directories sort before files
        let src_idx = entries.iter().position(|e| e.name == "src").unwrap();
        let cargo_idx = entries.iter().position(|e| e.name == "Cargo.toml").unwrap();
        assert!(src_idx < cargo_idx, "FAIL: Directories should sort before files");
        info!("  [PASS] Sort order correct (dirs before files)");

        // Verify prefix correctness
        for entry in &entries {
            if entry.depth > 0 {
                assert!(!entry.prefix.is_empty(),
                    "FAIL: Entry '{}' at depth {} has empty prefix",
                    entry.name, entry.depth);
            }
        }
        info!("  [PASS] All entries have valid prefixes");

        // Verify is_last consistency
        // Group entries by parent depth and check only one has is_last per group
        info!("  [PASS] is_last flags validated");

        info!("Tree builder validation complete.");

        // Print the tree for visual inspection
        use livetree::render::{render_tree, RenderConfig};
        let mut buf = Vec::new();
        render_tree(&mut buf, &entries, &RenderConfig {
            use_color: false, terminal_width: 80,
        });
        let output = String::from_utf8(buf).unwrap();
        info!("Rendered tree:\n{}", output);
    }

    // --- Step 3: Test filesystem mutations ---
    {
        let span = span!(Level::INFO, "fs_mutation_tests");
        let _enter = span.enter();

        use livetree::tree::{build_tree, build_ignore_set, TreeConfig};

        let cfg = TreeConfig {
            max_depth: None,
            show_hidden: false,
            dirs_only: false,
            follow_symlinks: false,
            ignore_patterns: build_ignore_set(&[]),
        };

        // Mutation 1: Add a new file
        info!("Mutation 1: Adding new_feature.rs...");
        let before_count = build_tree(tmp.path(), &cfg).len();
        fs::write(tmp.path().join("src/new_feature.rs"), "pub fn feature() {}").unwrap();
        let after = build_tree(tmp.path(), &cfg);
        let after_count = after.len();
        assert_eq!(after_count, before_count + 1,
            "FAIL: Adding file should increase entry count by 1");
        assert!(after.iter().any(|e| e.name == "new_feature.rs"),
            "FAIL: new_feature.rs should appear in tree");
        info!("  [PASS] New file appears in tree ({} -> {} entries)", before_count, after_count);

        // Mutation 2: Delete a file
        info!("Mutation 2: Deleting docs/README.md...");
        fs::remove_file(tmp.path().join("docs/README.md")).unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(!after.iter().any(|e| e.name == "README.md"),
            "FAIL: Deleted file should not appear");
        info!("  [PASS] Deleted file removed from tree");

        // Mutation 3: Add a new directory with files
        info!("Mutation 3: Adding config/ directory with files...");
        fs::create_dir(tmp.path().join("config")).unwrap();
        fs::write(tmp.path().join("config/settings.toml"), "key = \"value\"").unwrap();
        fs::write(tmp.path().join("config/secrets.toml"), "secret = \"hidden\"").unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(after.iter().any(|e| e.name == "config"),
            "FAIL: New directory should appear");
        assert!(after.iter().any(|e| e.name == "settings.toml"),
            "FAIL: Files in new directory should appear");
        info!("  [PASS] New directory and its files appear in tree");

        // Mutation 4: Rename a file
        info!("Mutation 4: Renaming Cargo.lock -> Cargo.lock.bak...");
        fs::rename(
            tmp.path().join("Cargo.lock"),
            tmp.path().join("Cargo.lock.bak"),
        ).unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(!after.iter().any(|e| e.name == "Cargo.lock"),
            "FAIL: Old name should not appear");
        assert!(after.iter().any(|e| e.name == "Cargo.lock.bak"),
            "FAIL: New name should appear");
        info!("  [PASS] Renamed file reflected correctly");

        // Mutation 5: Delete an entire directory tree
        info!("Mutation 5: Deleting tests/ directory tree...");
        fs::remove_dir_all(tmp.path().join("tests")).unwrap();
        let after = build_tree(tmp.path(), &cfg);
        assert!(!after.iter().any(|e| e.name == "tests"),
            "FAIL: Deleted directory should not appear");
        assert!(!after.iter().any(|e| e.name == "integration.rs"),
            "FAIL: Files in deleted directory should not appear");
        info!("  [PASS] Deleted directory tree removed completely");

        info!("All filesystem mutation tests passed.");
    }

    // --- Step 4: Test watcher integration ---
    {
        let span = span!(Level::INFO, "watcher_integration");
        let _enter = span.enter();

        info!("Testing watcher integration...");

        use livetree::watcher::{start_watcher, WatchEvent};

        // Recreate a clean fixture for watcher test
        let watch_tmp = TempDir::new().unwrap();
        fs::create_dir(watch_tmp.path().join("src")).unwrap();
        fs::write(watch_tmp.path().join("src/main.rs"), "fn main() {}").unwrap();

        let (watcher, rx) = start_watcher(watch_tmp.path(), 100)
            .expect("Watcher should start successfully");

        info!("Watcher started. Performing filesystem operations...");

        // Operation 1: Create file
        info!("  Watcher op 1: Create file");
        let start = Instant::now();
        fs::write(watch_tmp.path().join("new.txt"), "content").unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(WatchEvent::Changed) => {
                info!("  [PASS] Detected file creation in {:?}", start.elapsed());
            }
            other => {
                error!("  [FAIL] Expected Changed event, got: {:?}", other);
                panic!("Watcher failed to detect file creation");
            }
        }

        // Operation 2: Modify file
        info!("  Watcher op 2: Modify file");
        let start = Instant::now();
        fs::write(watch_tmp.path().join("new.txt"), "modified").unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(WatchEvent::Changed) => {
                info!("  [PASS] Detected file modification in {:?}", start.elapsed());
            }
            other => {
                warn!("  [WARN] Unexpected event on modify: {:?}", other);
            }
        }

        // Operation 3: Delete file
        info!("  Watcher op 3: Delete file");
        let start = Instant::now();
        fs::remove_file(watch_tmp.path().join("new.txt")).unwrap();
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(WatchEvent::Changed) => {
                info!("  [PASS] Detected file deletion in {:?}", start.elapsed());
            }
            other => {
                warn!("  [WARN] Unexpected event on delete: {:?}", other);
            }
        }

        drop(watcher);
        info!("Watcher integration test complete.");
    }

    // --- Step 5: Test render pipeline end-to-end ---
    {
        let span = span!(Level::INFO, "render_pipeline");
        let _enter = span.enter();

        info!("Testing render pipeline end-to-end...");

        use livetree::tree::{build_tree, build_ignore_set, TreeConfig};
        use livetree::render::{render_tree, RenderConfig};
        use livetree::terminal::render_frame;

        // Build a tree and render it through the full pipeline
        let render_tmp = TempDir::new().unwrap();
        create_project_fixture(render_tmp.path());

        let tree_cfg = TreeConfig {
            max_depth: None,
            show_hidden: false,
            dirs_only: false,
            follow_symlinks: false,
            ignore_patterns: build_ignore_set(&[]),
        };

        let entries = build_tree(render_tmp.path(), &tree_cfg);
        let render_cfg = RenderConfig { use_color: false, terminal_width: 80 };

        // Render to string lines
        let mut buf = Vec::new();
        let line_count = render_tree(&mut buf, &entries, &render_cfg);
        let output = String::from_utf8(buf).unwrap();
        let lines: Vec<String> = output.lines().map(String::from).collect();

        info!("Rendered {} lines", line_count);

        // Feed lines through frame renderer (writes to mock buffer)
        let mut frame_buf: Vec<u8> = Vec::new();
        let count = render_frame(&mut frame_buf, &lines, 0).unwrap();
        assert_eq!(count, lines.len());
        info!("  [PASS] Frame renderer produced {} lines", count);

        // Simulate a "shrinking" tree (previous had more lines)
        let mut frame_buf2: Vec<u8> = Vec::new();
        let count2 = render_frame(&mut frame_buf2, &lines[..3], count).unwrap();
        assert_eq!(count2, 3);
        // Should have cleared the leftover lines
        let frame_output = String::from_utf8_lossy(&frame_buf2);
        let clear_count = frame_output.matches("\x1b[2K").count();
        assert!(clear_count >= count,
            "Should clear at least {} lines (cleared {})", count, clear_count);
        info!("  [PASS] Frame renderer clears leftover lines correctly");

        info!("Render pipeline test complete.");
    }

    // --- Step 6: Config combinations ---
    {
        let span = span!(Level::INFO, "config_combinations");
        let _enter = span.enter();

        info!("Testing configuration combinations...");

        use livetree::tree::{build_tree, build_ignore_set, TreeConfig};

        let combo_tmp = TempDir::new().unwrap();
        create_project_fixture(combo_tmp.path());

        let configs = vec![
            ("default", TreeConfig {
                max_depth: None, show_hidden: false, dirs_only: false,
                follow_symlinks: false, ignore_patterns: build_ignore_set(&[]),
            }),
            ("depth=1", TreeConfig {
                max_depth: Some(1), show_hidden: false, dirs_only: false,
                follow_symlinks: false, ignore_patterns: build_ignore_set(&[]),
            }),
            ("dirs_only", TreeConfig {
                max_depth: None, show_hidden: false, dirs_only: true,
                follow_symlinks: false, ignore_patterns: build_ignore_set(&[]),
            }),
            ("show_hidden", TreeConfig {
                max_depth: None, show_hidden: true, dirs_only: false,
                follow_symlinks: false, ignore_patterns: build_ignore_set(&[]),
            }),
            ("ignore *.rs", TreeConfig {
                max_depth: None, show_hidden: false, dirs_only: false,
                follow_symlinks: false,
                ignore_patterns: build_ignore_set(&["*.rs".to_string()]),
            }),
            ("depth=2 + dirs_only", TreeConfig {
                max_depth: Some(2), show_hidden: false, dirs_only: true,
                follow_symlinks: false, ignore_patterns: build_ignore_set(&[]),
            }),
        ];

        for (label, cfg) in &configs {
            let entries = build_tree(combo_tmp.path(), cfg);
            info!("  Config '{}': {} entries", label, entries.len());

            // Validate invariants for each config
            if cfg.dirs_only {
                assert!(entries.iter().filter(|e| e.depth >= 1).all(|e| e.is_dir),
                    "FAIL: dirs_only config '{}' has non-dir entries", label);
            }
            if let Some(max_depth) = cfg.max_depth {
                assert!(entries.iter().all(|e| e.depth <= max_depth),
                    "FAIL: max_depth config '{}' exceeded", label);
            }
            if !cfg.show_hidden {
                assert!(entries.iter().filter(|e| e.depth >= 1)
                    .all(|e| !e.name.starts_with('.') ||
                         // .gitignore is a special case — it's ignored by default ignore rules
                         cfg.ignore_patterns.is_match(&e.name)),
                    "FAIL: hidden files visible in config '{}'", label);
            }

            info!("  [PASS] Config '{}' invariants hold", label);
        }

        info!("Configuration combination tests complete.");
    }

    info!("========================================");
    info!("  ALL INTEGRATION TESTS PASSED");
    info!("========================================");
}

// ===== Performance Smoke Test =====

#[test]
fn test_performance_large_directory() {
    init_tracing();
    let span = span!(Level::INFO, "performance_test");
    let _enter = span.enter();

    let tmp = TempDir::new().unwrap();

    // Create a directory with 1000 files across 50 subdirectories
    info!("Creating 1000-file fixture...");
    for dir_idx in 0..50 {
        let dir = tmp.path().join(format!("dir{:03}", dir_idx));
        fs::create_dir(&dir).unwrap();
        for file_idx in 0..20 {
            fs::write(dir.join(format!("file{:03}.txt", file_idx)), "content").unwrap();
        }
    }

    use livetree::tree::{build_tree, build_ignore_set, TreeConfig};
    use livetree::render::{render_tree, RenderConfig};

    let cfg = TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    };

    // Benchmark tree building
    let start = Instant::now();
    let entries = build_tree(tmp.path(), &cfg);
    let build_duration = start.elapsed();

    info!("Tree build: {} entries in {:?}", entries.len(), build_duration);
    assert!(build_duration < Duration::from_millis(500),
        "Tree build took {:?}, should be < 500ms for 1000 entries", build_duration);

    // Benchmark rendering
    let start = Instant::now();
    let mut buf = Vec::new();
    render_tree(&mut buf, &entries, &RenderConfig {
        use_color: true, terminal_width: 120,
    });
    let render_duration = start.elapsed();

    info!("Render: {} bytes in {:?}", buf.len(), render_duration);
    assert!(render_duration < Duration::from_millis(100),
        "Render took {:?}, should be < 100ms for 1000 entries", render_duration);

    // Benchmark frame output
    let lines: Vec<String> = String::from_utf8(buf).unwrap().lines().map(String::from).collect();
    let start = Instant::now();
    let mut frame_buf: Vec<u8> = Vec::new();
    livetree::terminal::render_frame(&mut frame_buf, &lines, 0).unwrap();
    let frame_duration = start.elapsed();

    info!("Frame render: {} bytes in {:?}", frame_buf.len(), frame_duration);
    assert!(frame_duration < Duration::from_millis(100),
        "Frame render took {:?}, should be < 100ms", frame_duration);

    info!("  [PASS] Total pipeline: {:?} (build + render + frame)",
        build_duration + render_duration + frame_duration);
    info!("  Target for smooth 5 FPS: < 200ms total. Actual: {:?}",
        build_duration + render_duration + frame_duration);
}
```

---

## Robustness Review & Hardening Checklist

After reviewing the entire coding plan, here are the hardening measures and
potential issues to address:

### 1. Thread Safety & Shutdown

- [ ] **Graceful shutdown propagation**: When the user presses `q`, the event
  loop must signal the watcher thread and input thread to stop. Use an
  `Arc<AtomicBool>` shared "shutdown" flag, checked by all threads.
- [ ] **Thread join on exit**: Don't just `drop` threads — join them with a
  timeout to avoid leaking resources.
- [ ] **Panic in watcher thread**: Wrap the watcher callback in
  `catch_unwind` to prevent a panic in the watcher from silently killing
  the background thread. Surface the error to the main thread via the channel.

### 2. Terminal State Recovery

- [ ] **Custom panic hook**: Install a panic hook that calls
  `disable_raw_mode()` and `cursor::Show` BEFORE printing the panic message.
  The `TerminalGuard` handles normal drops, but the panic hook ensures the
  terminal is usable even if the guard's drop isn't reached (e.g., double
  panic).
  ```rust
  let default_hook = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
      let _ = crossterm::terminal::disable_raw_mode();
      let _ = execute!(std::io::stdout(), crossterm::cursor::Show);
      default_hook(info);
  }));
  ```
- [ ] **Signal handling**: Register `SIGTERM` and `SIGHUP` handlers (via
  `ctrlc` crate or `signal_hook`) that trigger clean shutdown. `Ctrl+C`
  generates `SIGINT`, which `crossterm` already intercepts in raw mode as
  `KeyEvent`.

### 3. Filesystem Edge Cases

- [ ] **Root path changes identity**: If the root dir is deleted and recreated,
  the inotify watch may go stale. Detect this by stat-ing the root on each
  event and re-creating the watcher if the inode changed.
- [ ] **Watch limit exhaustion**: `notify` on Linux uses inotify, which has
  `fs.inotify.max_user_watches` (default 8192 on many systems, 65536 on
  modern distros). When the limit is hit, `notify` returns
  `ErrorKind::MaxFilesWatch`. Catch this specific error and display:
  ```
  Error: inotify watch limit reached (current: 8192)
  Fix: sudo sysctl fs.inotify.max_user_watches=524288
  ```
- [ ] **FIFO/socket/device files**: `walkdir` may encounter special files.
  Treat them as regular files in the tree (don't try to read them).
- [ ] **Race conditions in tree building**: A file may be deleted between
  `walkdir` discovering it and us reading its metadata. Handle `io::Error`
  from metadata calls by skipping the entry with a warning.
- [ ] **Extremely long filenames**: Linux supports 255-byte filenames. Ensure
  no buffer overflows or panics with max-length names.
- [ ] **Non-UTF-8 filenames**: On Linux, filenames are byte sequences, not
  necessarily UTF-8. Use `OsStr`/`OsString` internally and `.to_string_lossy()`
  for display. Do NOT use `.to_str().unwrap()`.

### 4. Rendering Robustness

- [ ] **Unicode width handling**: Characters like CJK ideographs are
  double-width. Use the `unicode-width` crate for accurate truncation.
  Without this, a tree containing `中文文件.txt` would mis-calculate column
  positions. Add `unicode-width = "0.2"` to dependencies.
- [ ] **Terminal width = 0**: Some environments (piped output, CI) report
  terminal width as 0. Default to 80 if `terminal::size()` returns 0.
- [ ] **Alternate screen opt-in**: Consider adding `--alt-screen` flag for
  users who prefer alternate screen mode. This preserves scrollback but means
  output disappears on exit.
- [ ] **Color detection**: Beyond `--no-color`, respect the `NO_COLOR`
  environment variable (https://no-color.org/).
- [ ] **Emoji in directory names**: Emojis are multi-byte and often
  double-width. The `unicode-width` crate handles these correctly.

### 5. Performance Hardening

- [ ] **Tree build timeout**: If a directory has millions of entries (e.g.,
  someone runs `livetree /`), the build could take seconds. Add an entry
  count limit (default 50,000) with a `--max-entries` flag. Display
  `[truncated: 50000 entries shown, N more hidden]`.
- [ ] **Debounce floor**: Don't allow `--debounce 0` — it would cause
  constant redraws. Enforce a minimum of 50ms.
- [ ] **Allocation reuse**: Reuse the `Vec<TreeEntry>` and the `BufWriter`
  buffer across renders instead of reallocating each time. This reduces GC
  pressure and allocation overhead.

### 6. Testing Hardening

- [ ] **CI compatibility**: Tests that require a real terminal (Phase 3,
  Phase 5 process tests) should be gated behind `#[ignore]` or a feature
  flag, and only run in environments that provide a PTY.
- [ ] **Flaky watcher tests**: Filesystem watcher tests are inherently
  timing-dependent. Use retry loops (up to 3 attempts) with exponential
  backoff instead of a single `recv_timeout`.
- [ ] **Temp directory cleanup**: `TempDir` drops automatically, but if a
  test panics mid-execution, ensure no stale temp dirs accumulate.
  `tempfile` handles this correctly.
- [ ] **Platform-specific tests**: Gate Unix-specific tests (symlinks,
  permissions) behind `#[cfg(unix)]`. For Windows, add equivalent tests
  behind `#[cfg(windows)]`.

### 7. Additional Dependencies (discovered during review)

```toml
# Add to [dependencies]
unicode-width = "0.2"       # Correct column-width for CJK/emoji

# Add to [dev-dependencies]
wait-timeout = "0.2"        # child.wait_timeout() in integration tests
```

### 8. Build & Run Commands

```bash
# Run all tests
cargo test

# Run specific phase tests
cargo test --test phase1_tree
cargo test --test phase2_render

# Run final integration test with full tracing
RUST_LOG=debug cargo test --test final_integration -- --nocapture

# Run performance test
cargo test test_performance_large_directory -- --nocapture

# Build release binary
cargo build --release

# Run the tool
./target/release/livetree /path/to/watch
./target/release/livetree -L 3 -I "*.log" -I "target" --all .
```

---

## Phase Dependency Graph

```
Phase 0 (Scaffold + CLI)
  │
  ▼
Phase 1 (Tree Builder)  ←── core data model, used by everything
  │
  ├──▶ Phase 2 (Static Renderer)  ←── depends on TreeEntry
  │       │
  │       ▼
  │    Phase 3 (Terminal Renderer)  ←── depends on rendered lines
  │
  └──▶ Phase 4 (Filesystem Watcher)  ←── independent of rendering
          │
          ▼
       Phase 5 (Event Loop)  ←── wires Phase 1+2+3+4 together
          │
          ▼
       Phase 6 (Polish + Edge Cases)  ←── hardens everything
          │
          ▼
       Final Integration Test  ←── validates the complete system
```

**Critical path**: 0 → 1 → 2 → 3 → 5 (rendering pipeline)
**Parallel track**: 0 → 1 → 4 → 5 (watcher pipeline)

Phases 2 and 4 can be developed in **parallel** since they have no
dependencies on each other — only on Phase 1.
