//! Terminal management via ratatui: init, restore, and size helpers.

use crossterm::terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};

/// The ratatui terminal type used throughout the application.
pub type Term = Terminal<CrosstermBackend<Stdout>>;

/// Initialize the terminal: enter alternate screen, enable raw mode,
/// hide cursor, and install a panic hook that restores state.
pub fn init() -> io::Result<Term> {
    let terminal = ratatui::init();
    Ok(terminal)
}

/// Restore the terminal: exit alternate screen, disable raw mode, show cursor.
pub fn restore() {
    ratatui::restore();
}

/// Get the current terminal size, falling back to (80, 24) if unavailable.
pub fn terminal_size() -> (u16, u16) {
    terminal::size().unwrap_or((80, 24))
}
