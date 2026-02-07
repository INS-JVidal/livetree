//! Main event loop: multiplexes filesystem events and keyboard input.

use crate::render::{format_status_bar, render_tree, RenderConfig};
use crate::terminal::{buffered_stdout, render_frame, terminal_size};
use crate::tree::{build_tree, TreeConfig};
use crate::watcher::WatchEvent;
use crossbeam_channel::{select, Receiver};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::io::{BufWriter, Stdout, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Holds mutable state for the application's render loop.
struct AppState<'a> {
    writer: BufWriter<Stdout>,
    prev_line_count: usize,
    last_change: Option<String>,
    use_color: bool,
    path: &'a Path,
    tree_config: &'a TreeConfig,
}

impl<'a> AppState<'a> {
    fn new(path: &'a Path, tree_config: &'a TreeConfig, use_color: bool) -> Self {
        Self {
            writer: buffered_stdout(),
            prev_line_count: 0,
            last_change: None,
            use_color,
            path,
            tree_config,
        }
    }

    /// Rebuild the tree and render a complete frame.
    fn render(&mut self, terminal_width: u16) {
        let entries = build_tree(self.path, self.tree_config);
        let entry_count = entries.len();

        let r_cfg = RenderConfig {
            use_color: self.use_color,
            terminal_width,
        };

        // Render tree lines to a buffer
        let mut line_buf = Vec::new();
        let _ = render_tree(&mut line_buf, &entries, &r_cfg);
        let text = String::from_utf8_lossy(&line_buf);
        let mut lines: Vec<String> = text.lines().map(String::from).collect();

        // Add status bar
        let status = format_status_bar(
            &self.path.to_string_lossy(),
            entry_count,
            self.last_change.as_deref(),
            terminal_width,
        );
        lines.push(String::new()); // blank separator line
        lines.push(status);

        let _ = render_frame(&mut self.writer, &lines, self.prev_line_count);
        let _ = self.writer.flush();
        self.prev_line_count = lines.len();
    }
}

/// Run the main application loop. Blocks until the user quits.
pub fn run(
    path: &Path,
    tree_config: &TreeConfig,
    render_config: &RenderConfig,
    fs_rx: Receiver<WatchEvent>,
) -> Result<(), String> {
    let shutdown = Arc::new(AtomicBool::new(false));

    // Spawn keyboard input reader
    let (key_tx, key_rx) = crossbeam_channel::unbounded();
    let shutdown_clone = shutdown.clone();
    let input_handle = thread::spawn(move || {
        while !shutdown_clone.load(Ordering::Relaxed) {
            // Poll with a timeout so we can check the shutdown flag
            if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    let _ = key_tx.send(evt);
                }
            }
        }
    });

    let mut state = AppState::new(path, tree_config, render_config.use_color);

    // Initial render
    let (term_width, _) = terminal_size();
    state.render(term_width);

    // Main event loop
    loop {
        select! {
            recv(fs_rx) -> msg => {
                match msg {
                    Ok(WatchEvent::Changed) => {
                        state.last_change = Some(chrono_lite_now());
                        let (w, _) = terminal_size();
                        state.render(w);
                    }
                    Ok(WatchEvent::RootDeleted) => {
                        let _ = render_frame(
                            &mut state.writer,
                            &[format!("Directory deleted: {}", path.display()),
                              "Exiting...".to_string()],
                            state.prev_line_count,
                        );
                        let _ = state.writer.flush();
                        break;
                    }
                    Ok(WatchEvent::Error(e)) => {
                        eprintln!("Watcher error: {}", e);
                    }
                    Err(_) => {
                        // Channel closed, watcher thread died
                        break;
                    }
                }
            }
            recv(key_rx) -> msg => {
                match msg {
                    Ok(Event::Key(KeyEvent { code: KeyCode::Char('q'), .. })) => break,
                    Ok(Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers, .. }))
                        if modifiers.contains(KeyModifiers::CONTROL) => break,
                    Ok(Event::Resize(w, _h)) => {
                        state.render(w);
                    }
                    _ => {}
                }
            }
        }
    }

    // Flush any remaining output before the TerminalGuard drops
    let _ = state.writer.flush();

    // Signal shutdown to input thread and wait
    shutdown.store(true, Ordering::Relaxed);
    let _ = input_handle.join();

    Ok(())
}

/// Simple timestamp without pulling in chrono.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Convert to HH:MM:SS (UTC — good enough for a status bar)
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
