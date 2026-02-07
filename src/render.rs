//! Tree rendering using ratatui Line/Span styling.

use crate::tree::TreeEntry;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

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
const DIM_STYLE: Style = Style::new().add_modifier(Modifier::DIM);

/// Convert a slice of `TreeEntry` into styled ratatui `Line` objects.
pub fn tree_to_lines(entries: &[TreeEntry], config: &RenderConfig) -> Vec<Line<'static>> {
    entries.iter().map(|e| entry_to_line(e, config)).collect()
}

/// Convert a single `TreeEntry` into a styled `Line`.
fn entry_to_line(entry: &TreeEntry, config: &RenderConfig) -> Line<'static> {
    let mut spans = Vec::new();

    // Prefix (tree-drawing characters)
    if !entry.prefix.is_empty() {
        if config.use_color {
            spans.push(Span::styled(entry.prefix.clone(), DIM_STYLE));
        } else {
            spans.push(Span::raw(entry.prefix.clone()));
        }
    }

    // Name + decorations
    if let Some(ref err) = entry.error {
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

/// Extract plain text from a `Line` (useful for testing).
pub fn line_to_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
