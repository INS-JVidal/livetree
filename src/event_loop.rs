use crate::render::{format_status_bar, render_tree, RenderConfig};
use crate::terminal::{buffered_stdout, render_frame, terminal_size};
use crate::tree::{build_tree, TreeConfig};
use crate::watcher::WatchEvent;
use crossbeam_channel::{select, Receiver};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

/// Run the main application loop. Blocks until the user quits.
pub fn run(
    path: &Path,
    tree_config: &TreeConfig,
    render_config: &RenderConfig,
    fs_rx: Receiver<WatchEvent>,
) -> Result<(), String> {
    let shutdown = Arc::new(AtomicBool::new(false));

    // Spawn keyboard input reader
    let (key_tx, key_rx) = crossbeam_channel::unbounded();
    let shutdown_clone = shutdown.clone();
    let input_handle = thread::spawn(move || {
        while !shutdown_clone.load(Ordering::Relaxed) {
            // Poll with a timeout so we can check the shutdown flag
            if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    let _ = key_tx.send(evt);
                }
            }
        }
    });

    let mut writer = buffered_stdout();
    let mut prev_line_count: usize = 0;
    let mut last_change: Option<String> = None;

    // Do initial render
    let (term_width, _term_height) = terminal_size();
    let use_color = render_config.use_color;

    let do_render =
        |writer: &mut std::io::BufWriter<std::io::Stdout>,
         tree_config: &TreeConfig,
         render_config_width: u16,
         use_color: bool,
         prev_lines: usize,
         last_change: &Option<String>,
         path: &Path|
         -> usize {
            let entries = build_tree(path, tree_config);
            let entry_count = entries.len();

            let r_cfg = RenderConfig {
                use_color,
                terminal_width: render_config_width,
            };

            // Render tree lines to a buffer
            let mut line_buf = Vec::new();
            render_tree(&mut line_buf, &entries, &r_cfg);
            let text = String::from_utf8_lossy(&line_buf);
            let mut lines: Vec<String> = text.lines().map(String::from).collect();

            // Add status bar
            let status = format_status_bar(
                &path.to_string_lossy(),
                entry_count,
                last_change.as_deref(),
                render_config_width,
            );
            lines.push(String::new()); // blank separator line
            lines.push(status);

            let _ = render_frame(writer, &lines, prev_lines);
            let _ = writer.flush();
            lines.len()
        };

    // Initial render
    prev_line_count = do_render(
        &mut writer,
        tree_config,
        term_width,
        use_color,
        prev_line_count,
        &last_change,
        path,
    );

    // Main event loop
    loop {
        select! {
            recv(fs_rx) -> msg => {
                match msg {
                    Ok(WatchEvent::Changed) => {
                        let now = chrono_lite_now();
                        last_change = Some(now);
                        let (w, _h) = terminal_size();
                        prev_line_count = do_render(
                            &mut writer, tree_config, w, use_color,
                            prev_line_count, &last_change, path,
                        );
                    }
                    Ok(WatchEvent::RootDeleted) => {
                        // Show message and exit
                        let _ = render_frame(
                            &mut writer,
                            &[format!("Directory deleted: {}", path.display()),
                              "Exiting...".to_string()],
                            prev_line_count,
                        );
                        let _ = writer.flush();
                        break;
                    }
                    Ok(WatchEvent::Error(e)) => {
                        // Show error but continue
                        eprintln!("Watcher error: {}", e);
                    }
                    Err(_) => {
                        // Channel closed, watcher thread died
                        break;
                    }
                }
            }
            recv(key_rx) -> msg => {
                match msg {
                    Ok(Event::Key(KeyEvent { code: KeyCode::Char('q'), .. })) => break,
                    Ok(Event::Key(KeyEvent { code: KeyCode::Char('c'), modifiers, .. }))
                        if modifiers.contains(KeyModifiers::CONTROL) => break,
                    Ok(Event::Resize(w, _h)) => {
                        prev_line_count = do_render(
                            &mut writer, tree_config, w, use_color,
                            prev_line_count, &last_change, path,
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    // Signal shutdown to input thread and wait
    shutdown.store(true, Ordering::Relaxed);
    let _ = input_handle.join();

    Ok(())
}

/// Simple timestamp without pulling in chrono.
fn chrono_lite_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Convert to HH:MM:SS (UTC — good enough for a status bar)
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}
