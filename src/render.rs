use crate::tree::TreeEntry;
use std::io::Write;
use unicode_width::UnicodeWidthStr;

pub struct RenderConfig {
    pub use_color: bool,
    pub terminal_width: u16,
}

// ANSI escape code constants
const BOLD_BLUE: &str = "\x1b[1;34m";
const CYAN: &str = "\x1b[36m";
const RED: &str = "\x1b[31m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

/// Render the tree to any writer. Returns the number of lines written.
pub fn render_tree<W: Write>(
    writer: &mut W,
    entries: &[TreeEntry],
    config: &RenderConfig,
) -> usize {
    let mut count = 0;
    for entry in entries {
        let line = format_entry(entry, config);
        writeln!(writer, "{}", line).unwrap();
        count += 1;
    }
    count
}

/// Render a single entry to a styled line (with or without ANSI codes).
pub fn format_entry(entry: &TreeEntry, config: &RenderConfig) -> String {
    let prefix = if config.use_color {
        colorize_prefix(&entry.prefix)
    } else {
        entry.prefix.clone()
    };

    let name_part = build_name_part(entry, config);

    let full_line = format!("{}{}", prefix, name_part);

    truncate_to_width(&full_line, config.terminal_width as usize)
}

/// Build the name portion of the line (colored name + optional symlink target or error).
fn build_name_part(entry: &TreeEntry, config: &RenderConfig) -> String {
    // Handle error entries
    if let Some(ref err) = entry.error {
        let error_text = format!("{} [{}]", entry.name, err);
        if config.use_color {
            return format!("{}{}{}", RED, error_text, RESET);
        } else {
            return error_text;
        }
    }

    // Build styled name
    let styled_name = if config.use_color {
        if entry.is_dir {
            format!("{}{}{}", BOLD_BLUE, entry.name, RESET)
        } else if entry.is_symlink {
            format!("{}{}{}", CYAN, entry.name, RESET)
        } else {
            entry.name.clone()
        }
    } else {
        entry.name.clone()
    };

    // For symlinks, append " -> target"
    if entry.is_symlink {
        let target = std::fs::read_link(&entry.path)
            .map(|t| t.to_string_lossy().to_string())
            .unwrap_or_else(|_| "?".to_string());
        let arrow_part = format!(" -> {}", target);
        if config.use_color {
            format!("{}{}{}{}", CYAN, entry.name, RESET, arrow_part)
        } else {
            format!("{}{}", styled_name, arrow_part)
        }
    } else {
        styled_name
    }
}

/// Wrap tree-drawing characters in the prefix with dim ANSI codes.
fn colorize_prefix(prefix: &str) -> String {
    if prefix.is_empty() {
        return String::new();
    }
    format!("{}{}{}", DIM, prefix, RESET)
}

/// Strip ANSI escape sequences from a string for display-width calculation.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        result.push(ch);
    }
    result
}

/// Truncate a string (which may contain ANSI codes) so that its display width
/// does not exceed `max_width`. If truncated, appends "...".
fn truncate_to_width(s: &str, max_width: usize) -> String {
    let plain = strip_ansi(s);
    let display_width = UnicodeWidthStr::width(plain.as_str());

    if display_width <= max_width {
        return s.to_string();
    }

    // We need to walk through the original string, tracking visible width,
    // and cut off when we'd exceed max_width - 1 (to leave room for "...").
    let ellipsis = "\u{2026}"; // ...
    let ellipsis_width = 1; // single-width character
    let target_width = max_width.saturating_sub(ellipsis_width);

    let mut result = String::with_capacity(s.len());
    let mut visible_width: usize = 0;
    let mut in_escape = false;

    for ch in s.chars() {
        if in_escape {
            result.push(ch);
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
            continue;
        }
        if ch == '\x1b' {
            in_escape = true;
            result.push(ch);
            continue;
        }

        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if visible_width + ch_width > target_width {
            break;
        }
        visible_width += ch_width;
        result.push(ch);
    }

    // Close any open ANSI sequences
    result.push_str(RESET);
    result.push_str(ellipsis);
    result
}

/// Render the status bar line.
pub fn format_status_bar(
    watched_path: &str,
    entry_count: usize,
    last_change: Option<&str>,
    terminal_width: u16,
) -> String {
    let change_text = match last_change {
        Some(ts) => format!("Last change: {}", ts),
        None => "No changes yet".to_string(),
    };

    let bar = format!(
        " Watching: {}  |  {} entries  |  {}",
        watched_path, entry_count, change_text
    );

    let width = terminal_width as usize;
    let display_width = UnicodeWidthStr::width(bar.as_str());

    if display_width <= width {
        // Pad to terminal width
        let padding = width - display_width;
        format!("{}{}", bar, " ".repeat(padding))
    } else {
        // Truncate
        truncate_to_width(&bar, width)
    }
}
