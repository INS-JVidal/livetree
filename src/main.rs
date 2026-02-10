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
    };

    let (term_width, _) = terminal::terminal_size();
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
