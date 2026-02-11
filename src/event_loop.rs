//! Main event loop: multiplexes filesystem events and keyboard input,
//! rendering via ratatui's immediate-mode draw loop.

use crate::highlight::HighlightTracker;
use crate::render::{help_bar_line, status_bar_line, tree_to_lines, RenderConfig, truncation_line};
use crate::terminal::Term;
use crate::tree::{TreeBuilder, TreeConfig, TreeSnapshot, WalkdirTreeBuilder};
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

/// Tracks scrolling state (offset + total lines) for the tree view.
struct ScrollState {
    offset: usize,
    total_lines: usize,
}

impl ScrollState {
    fn new() -> Self {
        Self {
            offset: 0,
            total_lines: 0,
        }
    }

    fn update_total_and_clamp(&mut self, total_lines: usize, view_height: usize) {
        self.total_lines = total_lines;
        if self.total_lines > view_height {
            let max_scroll = self.total_lines.saturating_sub(view_height);
            if self.offset > max_scroll {
                self.offset = max_scroll;
            }
        } else {
            self.offset = 0;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    fn scroll_down(&mut self, n: usize) {
        self.offset = self.offset.saturating_add(n);
    }

    fn scroll_home(&mut self) {
        self.offset = 0;
    }

    fn scroll_end(&mut self) {
        self.offset = usize::MAX;
    }

    fn offset(&self) -> usize {
        self.offset
    }
}

/// Holds mutable state for the application's render loop.
struct AppState<'a> {
    terminal: Term,
    last_change: Option<String>,
    use_color: bool,
    path: &'a Path,
    tree_config: &'a TreeConfig,
    /// Scroll state for the tree view.
    scroll: ScrollState,
    /// Tracks recently changed paths with per-entry expiration.
    highlights: HighlightTracker,
    /// Current highlight duration in whole seconds (0 disables highlighting).
    highlight_duration_secs: u64,
    /// Cached tree snapshot; invalidated on WatchEvent::Changed to avoid rebuild on every key.
    tree_cache: Option<TreeSnapshot>,
    /// Strategy for building the tree (allows swapping/mocking).
    tree_builder: &'a dyn TreeBuilder,
}

impl<'a> AppState<'a> {
    fn new(
        terminal: Term,
        path: &'a Path,
        tree_config: &'a TreeConfig,
        use_color: bool,
        tree_builder: &'a dyn TreeBuilder,
    ) -> Self {
        Self {
            terminal,
            last_change: None,
            use_color,
            path,
            tree_config,
            scroll: ScrollState::new(),
            highlights: HighlightTracker::new(std::time::Duration::from_secs(3)),
            highlight_duration_secs: 3,
            tree_cache: None,
            tree_builder,
        }
    }

    /// Rebuild the tree (if cache invalidated) and render a complete frame via ratatui.
    fn render(&mut self) {
        // Prune expired highlights and get the active set
        let now = Instant::now();
        let active_highlights = self.highlights.active_set(now);

        if self.tree_cache.is_none() {
            self.tree_cache = Some(self.tree_builder.build_tree(self.path, self.tree_config));
        }
        let Some(snapshot) = self.tree_cache.as_ref() else {
            return;
        };
        let entry_count_total = snapshot.total_entries;
        let entry_count_shown = snapshot.entries.len();

        let (term_width, area_height) = self
            .terminal
            .size()
            .map(|s| (s.width, s.height))
            .unwrap_or((80, 24));

        let r_cfg = RenderConfig {
            use_color: self.use_color,
            terminal_width: term_width,
        };

        let mut tree_lines = tree_to_lines(&snapshot.entries, &r_cfg, &active_highlights);
        let truncated = entry_count_total > entry_count_shown;
        if truncated {
            tree_lines.push(truncation_line(entry_count_shown, entry_count_total));
        }
        let tree_area_height = area_height.saturating_sub(2) as usize;
        self.scroll
            .update_total_and_clamp(tree_lines.len(), tree_area_height);

        let scroll_offset = self.scroll.offset();

        // Build status bar
        let display_count = if truncated {
            format!(
                "showing {} of {} entries (truncated)",
                entry_count_shown, entry_count_total
            )
        } else if self.scroll.total_lines > tree_area_height {
            format!(
                "{} entries ({} visible, scroll {}/{})",
                entry_count_total,
                tree_area_height.min(self.scroll.total_lines),
                scroll_offset + 1,
                self.scroll.total_lines.saturating_sub(tree_area_height) + 1,
            )
        } else {
            format!("{} entries", entry_count_total)
        };
        let path_str = format_watched_path(self.path);
        let status = status_bar_line(&path_str, &display_count, self.last_change.as_deref());

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
            let tree_widget = Paragraph::new(tree_lines).scroll((scroll_offset as u16, 0));
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
        self.scroll.scroll_up(n);
    }

    /// Scroll down by `n` lines.
    fn scroll_down(&mut self, n: usize) {
        self.scroll.scroll_down(n);
    }

    /// Scroll to top.
    fn scroll_home(&mut self) {
        self.scroll.scroll_home();
    }

    /// Scroll to bottom.
    fn scroll_end(&mut self) {
        self.scroll.scroll_end();
    }

    /// Get the visible tree area height (minus status bar + help bar).
    fn visible_height(&self) -> usize {
        let h = self.terminal.size().map(|s| s.height).unwrap_or(24);
        h.saturating_sub(2) as usize
    }
}

/// Internal implementation of the main loop, parameterized over a `TreeBuilder`.
fn run_with_tree_builder(
    terminal: Term,
    path: &Path,
    tree_config: &TreeConfig,
    render_config: &RenderConfig,
    fs_rx: Receiver<WatchEvent>,
    tree_builder: &dyn TreeBuilder,
    quiet: bool,
) {
    let shutdown = Arc::new(AtomicBool::new(false));
    let interrupted = Arc::new(AtomicBool::new(false));

    {
        let interrupted = interrupted.clone();
        let _ = ctrlc::set_handler(move || {
            interrupted.store(true, Ordering::SeqCst);
        });
    }

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

    let mut state = AppState::new(
        terminal,
        path,
        tree_config,
        render_config.use_color,
        tree_builder,
    );

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
                        // Highlight both files and directories; parent directories may also change.
                        let now = Instant::now();
                        for p in paths.into_iter() {
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
                        if !quiet {
                            eprintln!("Watcher error: {}", e);
                        }
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
                            KeyCode::Char('+') => {
                                // Increase highlight duration by 1s, saturating at a reasonable upper bound.
                                if state.highlight_duration_secs < 3600 {
                                    state.highlight_duration_secs += 1;
                                    state
                                        .highlights
                                        .set_duration(std::time::Duration::from_secs(
                                            state.highlight_duration_secs,
                                        ));
                                }
                                state.render();
                            }
                            KeyCode::Char('-') => {
                                // Decrease highlight duration by 1s, clamped at 0 (disable).
                                if state.highlight_duration_secs > 0 {
                                    state.highlight_duration_secs -= 1;
                                    state
                                        .highlights
                                        .set_duration(std::time::Duration::from_secs(
                                            state.highlight_duration_secs,
                                        ));
                                } else {
                                    state.highlights.set_duration(std::time::Duration::from_secs(0));
                                }
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
            default(std::time::Duration::from_millis(100)) => {
                if interrupted.load(Ordering::SeqCst) {
                    break;
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

/// Format the watched path for status bar display, collapsing the user's home
/// directory to `~` when applicable.
fn format_watched_path(path: &Path) -> String {
    let raw = path.to_string_lossy().to_string();
    let home = std::env::var("HOME").ok();
    let Some(home_str) = home else {
        return raw;
    };

    let home_path = std::path::Path::new(&home_str);
    if path == home_path {
        return "~".to_string();
    }
    if let Ok(stripped) = path.strip_prefix(home_path) {
        let rest = stripped.to_string_lossy();
        if rest.is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", rest)
        }
    } else {
        raw
    }
}

/// Run the main application loop with the default `WalkdirTreeBuilder`.
/// Blocks until the user quits.
pub fn run(
    terminal: Term,
    path: &Path,
    tree_config: &TreeConfig,
    render_config: &RenderConfig,
    fs_rx: Receiver<WatchEvent>,
    quiet: bool,
) {
    let default_builder = WalkdirTreeBuilder;
    run_with_tree_builder(
        terminal,
        path,
        tree_config,
        render_config,
        fs_rx,
        &default_builder,
        quiet,
    );
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
