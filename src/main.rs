mod cli;
mod event_loop;
mod render;
mod terminal;
mod tree;
mod watcher;

use clap::Parser;
use cli::Args;
use render::RenderConfig;
use tree::{build_ignore_set, TreeConfig};

fn main() {
    let args = Args::parse().validated();

    let path = args.path.canonicalize().unwrap_or_else(|e| {
        eprintln!("livetree: {}: {}", args.path.display(), e);
        std::process::exit(1);
    });

    if !path.is_dir() {
        eprintln!("livetree: {}: Not a directory", path.display());
        std::process::exit(1);
    }

    // Install panic hook for terminal safety
    terminal::install_panic_hook();

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

    // Start filesystem watcher
    let (_debouncer, fs_rx) = watcher::start_watcher(&path, args.debounce_ms).unwrap_or_else(|e| {
        eprintln!("livetree: failed to start watcher: {}", e);
        std::process::exit(1);
    });

    // Enter raw mode with RAII guard
    let _guard = terminal::TerminalGuard::new().unwrap_or_else(|e| {
        eprintln!("livetree: failed to initialize terminal: {}", e);
        std::process::exit(1);
    });

    // Run the main event loop (blocks until quit)
    if let Err(e) = event_loop::run(&path, &tree_config, &render_config, fs_rx) {
        eprintln!("livetree: {}", e);
        std::process::exit(1);
    }
}
