//! Terminal management: raw mode RAII guard, frame rendering, and panic hook.

use crossterm::{cursor, execute, queue, terminal};
use std::io::{self, Stdout, Write};

/// RAII guard that restores terminal state on drop (even on panic).
pub struct TerminalGuard {
    _private: (), // prevent construction outside this module
}

impl TerminalGuard {
    /// Enter raw mode and hide the cursor. Returns the guard.
    pub fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), cursor::Hide)?;
        Ok(TerminalGuard { _private: () })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::All),
            cursor::Show,
        );
    }
}

/// Install a custom panic hook that restores the terminal before printing
/// the panic message. Call this once at startup.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::All),
            cursor::Show,
        );
        default_hook(info);
    }));
}

/// Render a complete frame to any writer.
///
/// 1. Move cursor to (0, 0)
/// 2. For each line: clear the current line, write the content
/// 3. Clear leftover lines from the previous frame
/// 4. Never writes past `max_rows` to prevent terminal scrolling
/// 5. The caller is responsible for flushing (to achieve single-syscall output)
///
/// Returns the number of content lines written.
pub fn render_frame<W: Write>(
    writer: &mut W,
    lines: &[String],
    prev_line_count: usize,
    max_rows: usize,
) -> io::Result<usize> {
    // Move cursor home
    queue!(writer, cursor::MoveTo(0, 0))?;

    // Cap lines to max_rows to prevent scrolling.
    // We can write at most max_rows lines using max_rows-1 \r\n transitions
    // (the last line uses no \r\n to avoid scrolling off the bottom).
    let visible = lines.len().min(max_rows);

    for (i, line) in lines[..visible].iter().enumerate() {
        queue!(writer, terminal::Clear(terminal::ClearType::CurrentLine))?;
        if i < visible - 1 || visible < max_rows {
            // Not the last row of the terminal — safe to newline
            write!(writer, "{}\r\n", line)?;
        } else {
            // Last usable row — write without \r\n to prevent scroll
            write!(writer, "{}", line)?;
        }
    }

    // Clear leftover lines from previous frame, but never past max_rows
    let clear_up_to = prev_line_count.min(max_rows);
    for i in visible..clear_up_to {
        queue!(writer, terminal::Clear(terminal::ClearType::CurrentLine))?;
        if i < max_rows - 1 {
            write!(writer, "\r\n")?;
        }
    }

    Ok(visible)
}

/// Get the current terminal size, falling back to (80, 24) if unavailable.
pub fn terminal_size() -> (u16, u16) {
    terminal::size().unwrap_or((80, 24))
}

/// Create a BufWriter wrapping stdout with a generous buffer.
pub fn buffered_stdout() -> io::BufWriter<Stdout> {
    io::BufWriter::with_capacity(64 * 1024, io::stdout())
}
