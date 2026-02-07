//! Terminal management: raw mode RAII guard, frame rendering, and panic hook.

use crossterm::{cursor, queue, terminal};
use std::io::{self, Stdout, Write};

/// RAII guard that restores terminal state on drop (even on panic).
pub struct TerminalGuard {
    _private: (), // prevent construction outside this module
}

impl TerminalGuard {
    /// Enter alternate screen, raw mode, and hide the cursor. Returns the guard.
    pub fn new() -> io::Result<Self> {
        let mut stdout = io::stdout();
        queue!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
        stdout.flush()?;
        terminal::enable_raw_mode()?;
        Ok(TerminalGuard { _private: () })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = queue!(stdout, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = stdout.flush();
    }
}

/// Install a custom panic hook that restores the terminal before printing
/// the panic message. Call this once at startup.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = queue!(stdout, cursor::Show, terminal::LeaveAlternateScreen);
        let _ = stdout.flush();
        default_hook(info);
    }));
}

/// Render a complete frame to any writer.
///
/// 1. Move cursor to (0, 0)
/// 2. For each line: clear the current line, write the content
/// 3. Clear leftover lines from the previous frame
/// 4. The caller is responsible for flushing (to achieve single-syscall output)
///
/// Returns the number of lines written.
pub fn render_frame<W: Write>(
    writer: &mut W,
    lines: &[String],
    prev_line_count: usize,
) -> io::Result<usize> {
    // Move cursor home
    queue!(writer, cursor::MoveTo(0, 0))?;

    // Write each line, clearing as we go.
    // Use \r\n because in raw mode \n only moves down, doesn't return to column 0.
    for line in lines {
        queue!(writer, terminal::Clear(terminal::ClearType::CurrentLine))?;
        write!(writer, "{}\r\n", line)?;
    }

    // Clear any leftover lines from the previous frame
    for _ in lines.len()..prev_line_count {
        queue!(writer, terminal::Clear(terminal::ClearType::CurrentLine))?;
        write!(writer, "\r\n")?;
    }

    Ok(lines.len())
}

/// Get the current terminal size, falling back to (80, 24) if unavailable.
pub fn terminal_size() -> (u16, u16) {
    terminal::size().unwrap_or((80, 24))
}

/// Create a BufWriter wrapping stdout with a generous buffer.
pub fn buffered_stdout() -> io::BufWriter<Stdout> {
    io::BufWriter::with_capacity(64 * 1024, io::stdout())
}
