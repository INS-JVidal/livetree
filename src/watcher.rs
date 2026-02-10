//! Filesystem watcher using `notify-debouncer-full` with crossbeam channels.

use crossbeam_channel::{self, Receiver, Sender};
use notify::RecommendedWatcher;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, Debouncer, RecommendedCache};
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Events emitted by the filesystem watcher.
#[derive(Debug)]
pub enum WatchEvent {
    /// One or more files/directories changed, with their paths.
    Changed(Vec<PathBuf>),
    /// The watched root directory was deleted.
    RootDeleted,
    /// A watcher error occurred.
    Error(String),
}

/// Handle for the active watcher; must be kept alive while receiving events.
pub type WatcherHandle = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Trait abstraction for filesystem watching so it can be swapped or mocked.
#[allow(dead_code)]
pub trait FsWatcher {
    fn start(
        &self,
        path: &Path,
        debounce_ms: u64,
    ) -> Result<(WatcherHandle, Receiver<WatchEvent>), String>;
}

/// Start watching a directory. Returns the debouncer (must be kept alive!) and a receiver.
pub fn start_watcher(
    path: &Path,
    debounce_ms: u64,
) -> Result<(WatcherHandle, Receiver<WatchEvent>), String> {
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
                Ok(events) => {
                    // Only treat as root deleted when metadata says "not found"
                    match std::fs::metadata(&root_path) {
                        Ok(_) => {
                            let paths: Vec<PathBuf> = events
                                .iter()
                                .flat_map(|e| e.paths.iter().cloned())
                                .collect::<HashSet<_>>()
                                .into_iter()
                                .collect();
                            let _ = tx.send(WatchEvent::Changed(paths));
                        }
                        Err(e) if e.kind() == ErrorKind::NotFound => {
                            let _ = tx.send(WatchEvent::RootDeleted);
                        }
                        Err(e) => {
                            let _ = tx.send(WatchEvent::Error(format!("{}", e)));
                        }
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

/// Default watcher implementation backed by `notify` + `notify-debouncer-full`.
#[allow(dead_code)]
pub struct NotifyFsWatcher;

impl FsWatcher for NotifyFsWatcher {
    fn start(
        &self,
        path: &Path,
        debounce_ms: u64,
    ) -> Result<(WatcherHandle, Receiver<WatchEvent>), String> {
        start_watcher(path, debounce_ms)
    }
}
