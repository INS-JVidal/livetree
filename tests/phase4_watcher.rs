use livetree::watcher::{start_watcher, FsWatcher, NotifyFsWatcher, WatchEvent};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_watcher_detects_file_creation() {
    let dir = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    // Small sleep to let the watcher settle
    std::thread::sleep(Duration::from_millis(200));

    fs::write(dir.path().join("newfile.txt"), b"hello").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(event, WatchEvent::Changed(_)),
        "Expected Changed, got {:?}",
        event
    );

    drop(watcher);
}

#[test]
fn test_watcher_detects_file_deletion() {
    let dir = TempDir::new().unwrap();

    // Create file before watcher starts
    let file_path = dir.path().join("to_delete.txt");
    fs::write(&file_path, b"delete me").unwrap();

    let (watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    // Small sleep to let the watcher settle
    std::thread::sleep(Duration::from_millis(200));

    fs::remove_file(&file_path).unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(event, WatchEvent::Changed(_)),
        "Expected Changed, got {:?}",
        event
    );

    drop(watcher);
}

#[test]
fn test_watcher_detects_file_modification() {
    let dir = TempDir::new().unwrap();

    // Create file before watcher starts
    let file_path = dir.path().join("modify_me.txt");
    fs::write(&file_path, b"original content").unwrap();

    let (watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    // Small sleep to let the watcher settle
    std::thread::sleep(Duration::from_millis(200));

    fs::write(&file_path, b"modified content").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(event, WatchEvent::Changed(_)),
        "Expected Changed, got {:?}",
        event
    );

    drop(watcher);
}

#[test]
fn test_watcher_detects_directory_creation() {
    let dir = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    // Small sleep to let the watcher settle
    std::thread::sleep(Duration::from_millis(200));

    fs::create_dir(dir.path().join("new_subdir")).unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(event, WatchEvent::Changed(_)),
        "Expected Changed, got {:?}",
        event
    );

    drop(watcher);
}

#[test]
fn test_watcher_debounces_rapid_events() {
    let dir = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    // Small sleep to let the watcher settle
    std::thread::sleep(Duration::from_millis(200));

    // Create 50 files rapidly
    for i in 0..50 {
        fs::write(dir.path().join(format!("file_{}.txt", i)), b"data").unwrap();
    }

    // Wait for debounced events to arrive
    std::thread::sleep(Duration::from_millis(500));

    // Count all events that arrived
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }

    assert!(count > 0, "Expected at least one event, got {}", count);
    assert!(
        count < 10,
        "Expected fewer than 10 debounced events, got {}",
        count
    );

    drop(watcher);
}

#[test]
fn test_watcher_detects_nested_changes() {
    let dir = TempDir::new().unwrap();
    let (watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    // Small sleep to let the watcher settle
    std::thread::sleep(Duration::from_millis(200));

    // Create nested directories and a file deep inside
    let nested = dir.path().join("a").join("b").join("c");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("deep_file.txt"), b"deep content").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(event, WatchEvent::Changed(_)),
        "Expected Changed, got {:?}",
        event
    );

    drop(watcher);
}

#[test]
fn test_watcher_nonexistent_path_returns_error() {
    let result = start_watcher(std::path::Path::new("/nonexistent_path_xyz"), 100);
    assert!(result.is_err(), "Expected Err for nonexistent path, got Ok");
}

#[test]
fn test_watcher_changed_paths_contain_created_file() {
    let dir = TempDir::new().unwrap();
    let (_watcher, rx) = start_watcher(dir.path(), 100).unwrap();

    std::thread::sleep(Duration::from_millis(200));

    let target = dir.path().join("tracked.txt");
    fs::write(&target, b"content").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    match event {
        WatchEvent::Changed(paths) => {
            let path_set: std::collections::HashSet<PathBuf> = paths.into_iter().collect();
            assert!(
                path_set.contains(&target),
                "Changed paths should contain the created file. Got: {:?}",
                path_set
            );
        }
        other => panic!("Expected Changed, got {:?}", other),
    }
}

#[test]
fn test_notify_watcher_trait_start_works() {
    let dir = TempDir::new().unwrap();
    let watcher = NotifyFsWatcher;
    let (_handle, rx) = watcher.start(dir.path(), 100).unwrap();

    std::thread::sleep(Duration::from_millis(200));
    fs::write(dir.path().join("from_trait.txt"), b"hello").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        matches!(event, WatchEvent::Changed(_)),
        "Expected Changed event from trait watcher"
    );
}
