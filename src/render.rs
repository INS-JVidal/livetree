//! Tree rendering using ratatui Line/Span styling.

use crate::tree::TreeEntry;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashSet;
use std::path::PathBuf;

/// Configuration for the rendering pipeline.
pub struct RenderConfig {
    /// Whether to emit color styling.
    pub use_color: bool,
    /// Current terminal width in columns.
    pub terminal_width: u16,
}

// Color constants matching the original ANSI palette.
const DIR_STYLE: Style = Style::new()
    .fg(Color::Blue)
    .add_modifier(Modifier::BOLD);
const SYMLINK_STYLE: Style = Style::new().fg(Color::Cyan);
const ERROR_STYLE: Style = Style::new().fg(Color::Red);
const PREFIX_STYLE: Style = Style::new().fg(Color::White);
const CHANGED_STYLE: Style = Style::new()
    .fg(Color::Cyan)
    .add_modifier(Modifier::BOLD);

/// Convert a slice of `TreeEntry` into styled ratatui `Line` objects.
pub fn tree_to_lines(
    entries: &[TreeEntry],
    config: &RenderConfig,
    changed_paths: &HashSet<PathBuf>,
) -> Vec<Line<'static>> {
    entries
        .iter()
        .map(|e| entry_to_line(e, config, changed_paths))
        .collect()
}

/// Convert a single `TreeEntry` into a styled `Line`.
fn entry_to_line(
    entry: &TreeEntry,
    config: &RenderConfig,
    changed_paths: &HashSet<PathBuf>,
) -> Line<'static> {
    let is_changed = config.use_color && changed_paths.contains(&entry.path);
    let mut spans = Vec::new();

    // Prefix (tree-drawing characters)
    if !entry.prefix.is_empty() {
        if config.use_color {
            let prefix_style = PREFIX_STYLE;
            spans.push(Span::styled(entry.prefix.clone(), prefix_style));
        } else {
            spans.push(Span::raw(entry.prefix.clone()));
        }
    }

    // Name + decorations
    if is_changed {
        // Changed entries always get cyan bold, regardless of type
        spans.push(Span::styled(entry.name.clone(), CHANGED_STYLE));
        if entry.is_symlink {
            if let Some(ref target) = entry.symlink_target {
                spans.push(Span::styled(format!(" -> {}", target), CHANGED_STYLE));
            }
        }
    } else if let Some(ref err) = entry.error {
        let text = format!("{} [{}]", entry.name, err);
        if config.use_color {
            spans.push(Span::styled(text, ERROR_STYLE));
        } else {
            spans.push(Span::raw(text));
        }
    } else if entry.is_symlink {
        if config.use_color {
            spans.push(Span::styled(entry.name.clone(), SYMLINK_STYLE));
        } else {
            spans.push(Span::raw(entry.name.clone()));
        }
        if let Some(ref target) = entry.symlink_target {
            spans.push(Span::raw(format!(" -> {}", target)));
        }
    } else if entry.is_dir {
        if config.use_color {
            spans.push(Span::styled(entry.name.clone(), DIR_STYLE));
        } else {
            spans.push(Span::raw(entry.name.clone()));
        }
    } else {
        spans.push(Span::raw(entry.name.clone()));
    }

    Line::from(spans)
}

/// Build a styled status bar `Line`.
pub fn status_bar_line(
    watched_path: &str,
    entry_info: &str,
    last_change: Option<&str>,
    _width: u16,
) -> Line<'static> {
    let change_text = match last_change {
        Some(ts) => format!("Last change: {}", ts),
        None => "No changes yet".to_string(),
    };

    let text = format!(
        " Watching: {}  |  {}  |  {}",
        watched_path, entry_info, change_text
    );

    let style = Style::new()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);

    Line::from(Span::styled(text, style))
}

/// Build a help bar `Line` showing available keyboard shortcuts.
pub fn help_bar_line() -> Line<'static> {
    let text = " q: Sortir  |  r: Reset  |  ↑↓/jk: Scroll  |  PgUp/PgDn: Pàgina  |  Home/End";
    let style = Style::new().fg(Color::DarkGray);
    Line::from(Span::styled(text.to_string(), style))
}

/// Extract plain text from a `Line` (useful for testing).
pub fn line_to_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
