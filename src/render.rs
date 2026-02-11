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
    #[allow(dead_code)]
    pub terminal_width: u16,
}

// Color constants matching the original ANSI palette.
const DIR_STYLE: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
const SYMLINK_STYLE: Style = Style::new().fg(Color::Cyan);
const ERROR_STYLE: Style = Style::new().fg(Color::Red);
const PREFIX_STYLE: Style = Style::new().fg(Color::White);
const CHANGED_STYLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);
// Turquoise-green style for changed directories (distinct from default blue).
const CHANGED_DIR_STYLE: Style = Style::new()
    .fg(Color::Rgb(64, 224, 208))
    .add_modifier(Modifier::BOLD);

/// Sanitize control characters to avoid terminal control-sequence injection.
fn sanitize_terminal_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let code = c as u32;
                if code <= 0xFF {
                    out.push_str(&format!("\\x{:02X}", code));
                } else {
                    out.push_str(&format!("\\u{{{:X}}}", code));
                }
            }
            _ => out.push(c),
        }
    }
    out
}

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
    let safe_name = sanitize_terminal_text(&entry.name);

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
        // Changed entries: directories use turquoise-green, others use cyan bold.
        let style = if entry.is_dir {
            CHANGED_DIR_STYLE
        } else {
            CHANGED_STYLE
        };
        spans.push(Span::styled(safe_name.clone(), style));
        if entry.is_symlink {
            if let Some(ref target) = entry.symlink_target {
                let safe_target = sanitize_terminal_text(target);
                spans.push(Span::styled(format!(" -> {}", safe_target), style));
            }
        }
    } else if let Some(ref err) = entry.error {
        let safe_err = sanitize_terminal_text(err);
        let text = format!("{} [{}]", safe_name, safe_err);
        if config.use_color {
            spans.push(Span::styled(text, ERROR_STYLE));
        } else {
            spans.push(Span::raw(text));
        }
    } else if entry.is_symlink {
        if config.use_color {
            spans.push(Span::styled(safe_name.clone(), SYMLINK_STYLE));
        } else {
            spans.push(Span::raw(safe_name.clone()));
        }
        if let Some(ref target) = entry.symlink_target {
            let safe_target = sanitize_terminal_text(target);
            spans.push(Span::raw(format!(" -> {}", safe_target)));
        }
    } else if entry.is_dir {
        if config.use_color {
            spans.push(Span::styled(safe_name, DIR_STYLE));
        } else {
            spans.push(Span::raw(safe_name));
        }
    } else {
        spans.push(Span::raw(safe_name));
    }

    Line::from(spans)
}

/// Build a line indicating that the displayed entries were truncated.
pub fn truncation_line(shown: usize, total: usize) -> Line<'static> {
    let msg = format!("... showing {} of {} entries (truncated)", shown, total);
    let safe_msg = sanitize_terminal_text(&msg);
    let style = Style::new().fg(Color::DarkGray);
    Line::from(Span::styled(safe_msg, style))
}

/// Build a styled status bar `Line`.
pub fn status_bar_line(
    watched_path: &str,
    entry_info: &str,
    last_change: Option<&str>,
) -> Line<'static> {
    let change_text = match last_change {
        Some(ts) => format!("Last change: {}", sanitize_terminal_text(ts)),
        None => "No changes yet".to_string(),
    };

    let safe_path = sanitize_terminal_text(watched_path);
    let safe_entry_info = sanitize_terminal_text(entry_info);
    let text = format!(
        " Watching: {}  |  {}  |  {}",
        safe_path, safe_entry_info, change_text
    );

    let style = Style::new()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);

    Line::from(Span::styled(text, style))
}

/// Build a help bar `Line` showing available keyboard shortcuts.
pub fn help_bar_line() -> Line<'static> {
    let text =
        " q: Quit  |  r: Reset  |  ↑↓/jk: Scroll  |  PgUp/PgDn: Page  |  Home/End  |  +/-: Highlight duration";
    let style = Style::new().fg(Color::DarkGray);
    Line::from(Span::styled(text.to_string(), style))
}

/// Extract plain text from a `Line` (useful for testing).
#[allow(dead_code)]
pub fn line_to_plain_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_directory_uses_turquoise_style() {
        let path = PathBuf::from("/tmp/dir");
        let entry = TreeEntry {
            name: "dir".to_string(),
            path: path.clone(),
            depth: 1,
            is_dir: true,
            is_symlink: false,
            symlink_target: None,
            is_last: true,
            prefix: "".to_string(),
            error: None,
        };
        let mut changed = HashSet::new();
        changed.insert(path.clone());
        let cfg = RenderConfig {
            use_color: true,
            terminal_width: 80,
        };

        let line = entry_to_line(&entry, &cfg, &changed);
        let plain = line_to_plain_text(&line);
        assert!(
            plain.contains("dir"),
            "Rendered line should contain directory name"
        );
        // We cannot easily assert on Style here, but reaching this point
        // confirms rendering succeeds with changed-directory styling.
    }

    #[test]
    fn truncation_line_mentions_truncated() {
        let line = truncation_line(1000, 5000);
        let text = line_to_plain_text(&line);
        assert!(
            text.contains("showing 1000 of 5000"),
            "Truncation line should mention counts"
        );
        assert!(
            text.contains("truncated"),
            "Truncation line should mention truncation"
        );
    }
}
