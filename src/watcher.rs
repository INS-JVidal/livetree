use crossbeam_channel::{self, Receiver, Sender};
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, Debouncer, RecommendedCache};
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub enum WatchEvent {
    Changed,
    RootDeleted,
    Error(String),
}

/// Start watching a directory. Returns the debouncer (must be kept alive!) and a receiver.
pub fn start_watcher(
    path: &Path,
    debounce_ms: u64,
) -> Result<(Debouncer<RecommendedWatcher, RecommendedCache>, Receiver<WatchEvent>), String> {
    // Verify path exists before attempting to watch
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    let (tx, rx): (Sender<WatchEvent>, Receiver<WatchEvent>) = crossbeam_channel::unbounded();
    let root_path = path.to_path_buf();

    let mut debouncer = new_debouncer(
        Duration::from_millis(debounce_ms),
        None,
        move |result: Result<Vec<notify_debouncer_full::DebouncedEvent>, Vec<notify::Error>>| {
            match result {
                Ok(_events) => {
                    // Check if root path still exists
                    if std::fs::metadata(&root_path).is_err() {
                        let _ = tx.send(WatchEvent::RootDeleted);
                    } else {
                        let _ = tx.send(WatchEvent::Changed);
                    }
                }
                Err(errors) => {
                    for error in errors {
                        let _ = tx.send(WatchEvent::Error(format!("{}", error)));
                    }
                }
            }
        },
    )
    .map_err(|e| format!("Failed to create debouncer: {}", e))?;

    debouncer
        .watch(path, RecursiveMode::Recursive)
        .map_err(|e| format!("Failed to watch path {}: {}", path.display(), e))?;

    Ok((debouncer, rx))
}
