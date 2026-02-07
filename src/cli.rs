use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(name = "livetree", version, about = "Real-time directory tree watcher")]
pub struct Args {
    /// Directory to watch (default: current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Max display depth
    #[arg(short = 'L', long = "level")]
    pub max_depth: Option<usize>,

    /// Glob patterns to exclude (repeatable)
    #[arg(short = 'I', long = "ignore", action = clap::ArgAction::Append)]
    pub ignore: Vec<String>,

    /// Show hidden files (dotfiles)
    #[arg(short = 'a', long = "all")]
    pub show_hidden: bool,

    /// Only show directories
    #[arg(short = 'D', long = "dirs-only")]
    pub dirs_only: bool,

    /// Follow symbolic links
    #[arg(short = 'f', long = "follow-symlinks")]
    pub follow_symlinks: bool,

    /// Debounce interval in milliseconds (minimum 50)
    #[arg(long = "debounce", default_value = "200")]
    pub debounce_ms: u64,

    /// Disable colored output
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl Args {
    /// Enforce invariants after parsing.
    pub fn validated(mut self) -> Self {
        if self.debounce_ms < 50 {
            self.debounce_ms = 50;
        }
        // Respect NO_COLOR env var
        if std::env::var("NO_COLOR").is_ok() {
            self.no_color = true;
        }
        self
    }
}
