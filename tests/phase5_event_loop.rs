mod common;

use common::default_tree_config;
use livetree::render::RenderConfig;
use livetree::tree::build_tree;
use std::time::Duration;
use tempfile::TempDir;

/// Test the core logic: when a filesystem change occurs, rebuild+render
/// picks up the new state.
#[test]
fn test_rebuild_detects_new_file() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "").unwrap();

    let cfg = default_tree_config();
    let entries1 = build_tree(tmp.path(), &cfg);
    assert_eq!(entries1.iter().filter(|e| e.depth == 1).count(), 1);

    // Simulate filesystem change
    std::fs::write(tmp.path().join("b.txt"), "").unwrap();

    let entries2 = build_tree(tmp.path(), &cfg);
    assert_eq!(
        entries2.iter().filter(|e| e.depth == 1).count(),
        2,
        "Rebuild after change should show new file"
    );
}

/// Test that the render pipeline works end-to-end: build -> render -> frame.
#[test]
fn test_full_pipeline_build_render_frame() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join("src")).unwrap();
    std::fs::write(tmp.path().join("src/main.rs"), "").unwrap();
    std::fs::write(tmp.path().join("README.md"), "").unwrap();

    let cfg = default_tree_config();
    let entries = build_tree(tmp.path(), &cfg);

    let rcfg = RenderConfig {
        use_color: false,
        terminal_width: 80,
    };

    // Render to lines
    let mut buf = Vec::new();
    livetree::render::render_tree(&mut buf, &entries, &rcfg).unwrap();
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<String> = text.lines().map(String::from).collect();

    // Feed through frame renderer
    let mut frame_buf: Vec<u8> = Vec::new();
    let count = livetree::terminal::render_frame(&mut frame_buf, &lines, 0).unwrap();

    assert_eq!(count, lines.len());
    assert!(count >= 3, "Should have at least 3 lines (src, main.rs, README.md)");

    let output = String::from_utf8_lossy(&frame_buf);
    assert!(output.contains("src"));
    assert!(output.contains("main.rs"));
    assert!(output.contains("README.md"));
}

/// Test that the watcher + rebuild cycle works together.
#[test]
fn test_watcher_triggers_rebuild() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("initial.txt"), "").unwrap();

    let (watcher, rx) = livetree::watcher::start_watcher(tmp.path(), 100).unwrap();

    // Let watcher settle
    std::thread::sleep(Duration::from_millis(200));

    // Mutate filesystem
    std::fs::write(tmp.path().join("new.txt"), "content").unwrap();

    // Wait for event
    let event = rx.recv_timeout(Duration::from_secs(2));
    assert!(event.is_ok(), "Should receive watcher event");

    // Rebuild tree and verify
    let cfg = default_tree_config();
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"new.txt"), "Rebuilt tree should contain new file");
    assert!(names.contains(&"initial.txt"), "Rebuilt tree should still contain initial file");

    drop(watcher);
}
