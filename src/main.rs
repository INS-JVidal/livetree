#![forbid(unsafe_code)]
mod cli;
mod event_loop;
mod highlight;
mod render;
mod terminal;
mod tree;
mod watcher;

use anyhow::{Context, Result};
use clap::Parser;
use cli::Args;
use render::RenderConfig;
use tree::{build_ignore_set, TreeConfig};

fn main() {
    if let Err(e) = run_app() {
        eprintln!("livetree: {e:#}");
        std::process::exit(1);
    }
}

fn run_app() -> Result<()> {
    let args = Args::parse().validated();

    let path = args
        .path
        .canonicalize()
        .with_context(|| format!("{}: failed to resolve path", args.path.display()))?;

    anyhow::ensure!(path.is_dir(), "{}: Not a directory", path.display());

    // Build configs
    let tree_config = TreeConfig {
        max_depth: args.max_depth,
        show_hidden: args.show_hidden,
        dirs_only: args.dirs_only,
        follow_symlinks: args.follow_symlinks,
        ignore_patterns: build_ignore_set(&args.ignore),
        max_entries: Some(args.max_entries),
    };

    let (term_width, _) = terminal::terminal_size();

    // Optionally set the terminal (window/pane) title so multiplexers like Zellij
    // can display a meaningful name. The title is formatted as
    // "Live Tree <dir>", with HOME collapsed to "~", and truncated with an
    // ellipsis if it would exceed the terminal width.
    if !args.no_title {
        if let Some(title) = build_terminal_title(&path, term_width as usize) {
            use std::io::Write as _;
            let mut stdout = std::io::stdout();
            let _ = write!(stdout, "\x1b]0;{}\x07", title);
            let _ = stdout.flush();
        }
    }
    let render_config = RenderConfig {
        use_color: !args.no_color,
        terminal_width: term_width,
    };

    if args.verbose > 0 && !args.quiet {
        eprintln!(
            "livetree: watching {} (debounce={}ms, color={})",
            path.display(),
            args.debounce_ms,
            if render_config.use_color { "on" } else { "off" }
        );
    }

    // Start filesystem watcher
    let (_debouncer, fs_rx) = watcher::start_watcher(&path, args.debounce_ms)
        .map_err(anyhow::Error::msg)
        .context("failed to start watcher")?;

    // Initialize ratatui terminal (alternate screen, raw mode, panic hook)
    let term = terminal::init().context("failed to initialize terminal")?;

    // Run the main event loop (blocks until quit)
    event_loop::run(term, &path, &tree_config, &render_config, fs_rx, args.quiet);

    // Restore terminal state
    terminal::restore();
    Ok(())
}

/// Build a terminal title of the form "Live Tree <dir>", where <dir> is the
/// directory name only, truncated with a middle ellipsis so it does not exceed
/// `max_cols` characters.
fn build_terminal_title(path: &std::path::Path, max_cols: usize) -> Option<String> {
    if max_cols == 0 {
        return None;
    }

    let display = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    let raw_title = format!("Live Tree of {}", display);
    let sanitized = sanitize_title(&raw_title);
    Some(truncate_middle(&sanitized, max_cols))
}

/// Remove control characters that might interfere with terminal behavior.
fn sanitize_title(input: &str) -> String {
    input
        .chars()
        .filter(|c| {
            // Keep standard printable characters and space; drop other ASCII controls.
            !c.is_control() || *c == ' '
        })
        .collect()
}

/// Truncate a string in the middle with "..." so its length does not exceed
/// `max_cols`. If the string is already short enough, it is returned as-is.
fn truncate_middle(input: &str, max_cols: usize) -> String {
    if input.len() <= max_cols {
        return input.to_string();
    }
    if max_cols == 0 {
        return String::new();
    }
    if max_cols <= 3 {
        return ".".repeat(max_cols);
    }

    let ellipsis = "...";
    let keep = max_cols - ellipsis.len();
    let prefix_len = keep / 2 + keep % 2;
    let suffix_len = keep / 2;

    let prefix = &input[..prefix_len];
    let suffix = &input[input.len() - suffix_len..];

    format!("{prefix}{ellipsis}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn truncate_middle_short_strings_unchanged() {
        assert_eq!(truncate_middle("short", 10), "short");
    }

    #[test]
    fn truncate_middle_basic_case() {
        let s = "current_path_too_long";
        let truncated = truncate_middle(s, 16);
        assert_eq!(truncated, "current_...long");
        assert_eq!(truncated.len(), 16);
    }

    #[test]
    fn build_terminal_title_shows_dir_name_only() {
        let path = PathBuf::from("/home/testuser/projects/treewatch");
        let title = build_terminal_title(&path, 80).unwrap();
        assert_eq!(title, "Live Tree of treewatch");
    }
}
