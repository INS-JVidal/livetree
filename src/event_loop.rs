//! Main event loop: multiplexes filesystem events and keyboard input,
//! rendering via ratatui's immediate-mode draw loop.

use crate::highlight::HighlightTracker;
use crate::render::{help_bar_line, status_bar_line, tree_to_lines, RenderConfig};
use crate::terminal::Term;
use crate::tree::{build_tree, TreeConfig, TreeEntry};
use crate::watcher::WatchEvent;
use crossbeam_channel::{select, Receiver};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

/// Holds mutable state for the application's render loop.
struct AppState<'a> {
    terminal: Term,
    scroll_offset: usize,
    last_change: Option<String>,
    use_color: bool,
    path: &'a Path,
    tree_config: &'a TreeConfig,
    /// Total number of tree lines (for scroll clamping).
    total_lines: usize,
    /// Tracks recently changed paths with per-entry expiration.
    highlights: HighlightTracker,
    /// Cached tree entries; invalidated on WatchEvent::Changed to avoid rebuild on every key.
    tree_cache: Option<Vec<TreeEntry>>,
}

impl<'a> AppState<'a> {
    fn new(terminal: Term, path: &'a Path, tree_config: &'a TreeConfig, use_color: bool) -> Self {
        Self {
            terminal,
            scroll_offset: 0,
            last_change: None,
            use_color,
            path,
            tree_config,
            total_lines: 0,
            highlights: HighlightTracker::new(),
            tree_cache: None,
        }
    }

    /// Rebuild the tree (if cache invalidated) and render a complete frame via ratatui.
    fn render(&mut self) {
        // Prune expired highlights and get the active set
        let now = Instant::now();
        let active_highlights = self.highlights.active_set(now);

        if self.tree_cache.is_none() {
            self.tree_cache = Some(build_tree(self.path, self.tree_config));
        }
        let entries = self.tree_cache.as_ref().unwrap();
        let entry_count = entries.len();

        let (term_width, area_height) = self
            .terminal
            .size()
            .map(|s| (s.width, s.height))
            .unwrap_or((80, 24));

        let r_cfg = RenderConfig {
            use_color: self.use_color,
            terminal_width: term_width,
        };

        let tree_lines = tree_to_lines(&entries, &r_cfg, &active_highlights);
        self.total_lines = tree_lines.len();
        let tree_area_height = area_height.saturating_sub(2) as usize;
        if self.total_lines > tree_area_height {
            let max_scroll = self.total_lines.saturating_sub(tree_area_height);
            if self.scroll_offset > max_scroll {
                self.scroll_offset = max_scroll;
            }
        } else {
            self.scroll_offset = 0;
        }

        let scroll_offset = self.scroll_offset;

        // Build status bar
        let display_count = if self.total_lines > tree_area_height {
            format!(
                "{} entries ({} visible, scroll {}/{})",
                entry_count,
                tree_area_height.min(self.total_lines),
                scroll_offset + 1,
                self.total_lines.saturating_sub(tree_area_height) + 1,
            )
        } else {
            format!("{} entries", entry_count)
        };
        let path_str = self.path.to_string_lossy().to_string();
        let status = status_bar_line(
            &path_str,
            &display_count,
            self.last_change.as_deref(),
        );

        // Build help bar
        let help = help_bar_line();

        let _ = self.terminal.draw(|frame| {
            let area = frame.area();

            // Split: tree area, status bar (1 row), help bar (1 row)
            let chunks = Layout::vertical([
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(area);

            // Tree paragraph with scroll
            let tree_widget = Paragraph::new(tree_lines)
                .scroll((scroll_offset as u16, 0));
            frame.render_widget(tree_widget, chunks[0]);

            // Status bar
            let status_widget = Paragraph::new(status);
            frame.render_widget(status_widget, chunks[1]);

            // Help bar
            let help_widget = Paragraph::new(help);
            frame.render_widget(help_widget, chunks[2]);
        });
    }

    /// Render a message (e.g., "Directory deleted") and wait briefly.
    fn render_message(&mut self, lines: Vec<Line<'static>>) {
        let _ = self.terminal.draw(|frame| {
            let area = frame.area();
            let widget = Paragraph::new(lines);
            frame.render_widget(widget, area);
        });
    }

    /// Scroll up by `n` lines.
    fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    /// Scroll down by `n` lines.
    fn scroll_down(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(n);
        // render() will clamp
    }

    /// Scroll to top.
    fn scroll_home(&mut self) {
        self.scroll_offset = 0;
    }

    /// Scroll to bottom.
    fn scroll_end(&mut self) {
        // Set to large value; render() will clamp
        self.scroll_offset = usize::MAX;
    }

    /// Get the visible tree area height (minus status bar + help bar).
    fn visible_height(&self) -> usize {
        let h = self.terminal.size().map(|s| s.height).unwrap_or(24);
        h.saturating_sub(2) as usize
    }
}

/// Run the main application loop. Blocks until the user quits.
pub fn run(
    terminal: Term,
    path: &Path,
    tree_config: &TreeConfig,
    render_config: &RenderConfig,
    fs_rx: Receiver<WatchEvent>,
) {
    let shutdown = Arc::new(AtomicBool::new(false));

    // Spawn keyboard input reader
    let (key_tx, key_rx) = crossbeam_channel::unbounded();
    let shutdown_clone = shutdown.clone();
    let input_handle = thread::spawn(move || {
        while !shutdown_clone.load(Ordering::Relaxed) {
            if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    let _ = key_tx.send(evt);
                }
            }
        }
    });

    let mut state = AppState::new(terminal, path, tree_config, render_config.use_color);

    // Initial render
    state.render();

    // Main event loop
    loop {
        select! {
            recv(fs_rx) -> msg => {
                match msg {
                    Ok(WatchEvent::Changed(paths)) => {
                        state.last_change = Some(chrono_lite_now());
                        state.tree_cache = None; // invalidate so render() rebuilds tree
                        // Only highlight files, not directories (inotify
                        // reports parent dirs when their children change).
                        let now = Instant::now();
                        for p in paths.into_iter().filter(|p| !p.is_dir()) {
                            state.highlights.insert(p, now);
                        }
                        // Keep scroll position; render() will clamp if tree shrunk
                        state.render();
                    }
                    Ok(WatchEvent::RootDeleted) => {
                        state.render_message(vec![
                            Line::raw(format!("Directory deleted: {}", path.display())),
                            Line::raw("Exiting...".to_string()),
                        ]);
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
                    Ok(Event::Key(KeyEvent { code, modifiers, kind: KeyEventKind::Press, .. })) => {
                        match code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('r') => {
                                state.highlights.clear();
                                state.render();
                            }
                            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.scroll_up(1);
                                state.render();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.scroll_down(1);
                                state.render();
                            }
                            KeyCode::PageUp => {
                                let h = state.visible_height();
                                state.scroll_up(h);
                                state.render();
                            }
                            KeyCode::PageDown => {
                                let h = state.visible_height();
                                state.scroll_down(h);
                                state.render();
                            }
                            KeyCode::Home => {
                                state.scroll_home();
                                state.render();
                            }
                            KeyCode::End => {
                                state.scroll_end();
                                state.render();
                            }
                            _ => {}
                        }
                    }
                    Ok(Event::Resize(_, _)) => {
                        state.render();
                    }
                    _ => {}
                }
            }
        }
    }

    // Signal shutdown to input thread and wait
    shutdown.store(true, Ordering::Relaxed);
    if let Err(e) = input_handle.join() {
        std::panic::resume_unwind(e);
    }
}

/// Simple timestamp without pulling in chrono.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
