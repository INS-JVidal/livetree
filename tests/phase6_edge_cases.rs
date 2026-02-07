mod common;

use common::{default_tree_config, no_color_render_config, strip_ansi};
use livetree::render::{format_entry, render_tree, RenderConfig};
use livetree::tree::{build_ignore_set, build_tree, TreeConfig, TreeEntry};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn default_config() -> TreeConfig {
    default_tree_config()
}

fn no_color(width: u16) -> RenderConfig {
    no_color_render_config(width)
}

// --- Permission Denied ---

#[test]
#[cfg(unix)]
fn test_permission_denied_subdirectory() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let forbidden = tmp.path().join("forbidden");
    fs::create_dir(&forbidden).unwrap();
    fs::write(forbidden.join("secret.txt"), "").unwrap();
    // Remove read+execute permissions
    fs::set_permissions(&forbidden, fs::Permissions::from_mode(0o000)).unwrap();

    let entries = build_tree(tmp.path(), &default_config());

    // The forbidden directory should appear in tree
    let entry = entries.iter().find(|e| e.name == "forbidden");
    assert!(entry.is_some(), "forbidden dir should still appear in tree");

    // Its children should NOT appear (can't be read)
    let secret = entries.iter().find(|e| e.name == "secret.txt");
    assert!(
        secret.is_none(),
        "secret.txt should not be visible inside forbidden dir"
    );

    // Restore permissions for cleanup
    fs::set_permissions(&forbidden, fs::Permissions::from_mode(0o755)).unwrap();
}

// --- Symlink Loops ---

#[test]
#[cfg(unix)]
fn test_symlink_loop_handled() {
    let tmp = TempDir::new().unwrap();
    let dir_a = tmp.path().join("a");
    let dir_b = tmp.path().join("a/b");
    fs::create_dir_all(&dir_b).unwrap();
    // Create cycle: a/b/loop -> a
    std::os::unix::fs::symlink(&dir_a, dir_b.join("loop")).unwrap();

    let mut cfg = default_config();
    cfg.follow_symlinks = true;

    // This should NOT hang or panic
    let entries = build_tree(tmp.path(), &cfg);

    assert!(!entries.is_empty(), "Should produce output despite symlink loop");
    assert!(
        entries.len() < 100,
        "Symlink loop should not cause infinite traversal. Got {} entries",
        entries.len()
    );
}

// --- Terminal Too Narrow ---

#[test]
fn test_very_narrow_terminal() {
    let entry = TreeEntry {
        name: "very_long_filename_that_exceeds_width.rs".to_string(),
        path: PathBuf::from("very_long_filename_that_exceeds_width.rs"),
        depth: 1,
        is_dir: false,
        is_symlink: false,
        symlink_target: None,
        is_last: true,
        prefix: "\u{2514}\u{2500}\u{2500} ".to_string(),
        error: None,
    };

    let cfg = no_color(20);
    let line = format_entry(&entry, &cfg);
    // Strip ANSI and measure
    let plain = strip_ansi(&line);
    let display_width = unicode_width::UnicodeWidthStr::width(plain.as_str());
    assert!(
        display_width <= 20,
        "Line should fit in 20-char terminal. Got {} chars: {:?}",
        display_width,
        line
    );
}

// --- Terminal Width = 1 (extreme) ---

#[test]
fn test_terminal_width_1() {
    let entry = TreeEntry {
        name: "file.txt".to_string(),
        path: PathBuf::from("file.txt"),
        depth: 1,
        is_dir: false,
        is_symlink: false,
        symlink_target: None,
        is_last: true,
        prefix: "\u{2514}\u{2500}\u{2500} ".to_string(),
        error: None,
    };

    let cfg = no_color(1);
    // Should not panic
    let _line = format_entry(&entry, &cfg);
}

// --- Empty Root ---

#[test]
fn test_empty_root_directory() {
    let tmp = TempDir::new().unwrap();
    let entries = build_tree(tmp.path(), &default_config());

    let mut buf = Vec::new();
    let count = render_tree(&mut buf, &entries, &no_color(80)).unwrap();
    assert!(count <= 1, "Empty dir should produce at most 1 line");
}

// --- Nested Empty Directories ---

#[test]
fn test_nested_empty_directories() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("a/b/c")).unwrap();

    let entries = build_tree(tmp.path(), &default_config());
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a"));
    assert!(names.contains(&"b"));
    assert!(names.contains(&"c"));
}

// --- Mixed Symlinks ---

#[test]
#[cfg(unix)]
fn test_symlink_to_file_shows_arrow() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("real.txt"), "content").unwrap();
    std::os::unix::fs::symlink(
        tmp.path().join("real.txt"),
        tmp.path().join("link.txt"),
    )
    .unwrap();

    let entries = build_tree(tmp.path(), &default_config());
    let link = entries.iter().find(|e| e.name == "link.txt").unwrap();
    assert!(link.is_symlink);

    let cfg = no_color(120);
    let line = format_entry(link, &cfg);
    assert!(
        line.contains("->"),
        "Symlink should show target: {:?}",
        line
    );
}

// --- Render at Various Widths ---

#[test]
fn test_render_at_various_widths() {
    let entry = TreeEntry {
        name: "filename.txt".to_string(),
        path: PathBuf::from("filename.txt"),
        depth: 1,
        is_dir: false,
        is_symlink: false,
        symlink_target: None,
        is_last: true,
        prefix: "\u{2514}\u{2500}\u{2500} ".to_string(),
        error: None,
    };

    // Render at multiple widths — none should panic
    for width in [1, 5, 10, 20, 40, 80, 120, 200] {
        let cfg = no_color(width);
        let mut buf = Vec::new();
        render_tree(&mut buf, &[entry.clone()], &cfg).unwrap();
        // Just verify no panic
    }
}

// --- Non-UTF-8 Safety (lossy conversion) ---

#[test]
fn test_entry_with_special_characters() {
    let tmp = TempDir::new().unwrap();
    // Filenames with special but valid UTF-8 characters
    fs::write(tmp.path().join("café.txt"), "").unwrap();
    fs::write(tmp.path().join("日本語.md"), "").unwrap();
    fs::write(tmp.path().join("emoji-🎉.txt"), "").unwrap();

    let entries = build_tree(tmp.path(), &default_config());
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"café.txt"));
    assert!(names.contains(&"日本語.md"));
    assert!(names.contains(&"emoji-🎉.txt"));
}

// --- Large Directory ---

#[test]
fn test_large_directory_performance() {
    let tmp = TempDir::new().unwrap();
    for i in 0..500 {
        fs::write(tmp.path().join(format!("file_{:04}.txt", i)), "").unwrap();
    }

    let start = std::time::Instant::now();
    let entries = build_tree(tmp.path(), &default_config());
    let build_time = start.elapsed();

    assert_eq!(entries.len(), 500);
    assert!(
        build_time < std::time::Duration::from_millis(500),
        "Building 500-entry tree should be fast. Took {:?}",
        build_time
    );

    let start = std::time::Instant::now();
    let mut buf = Vec::new();
    render_tree(&mut buf, &entries, &no_color(80)).unwrap();
    let render_time = start.elapsed();

    assert!(
        render_time < std::time::Duration::from_millis(100),
        "Rendering 500 entries should be fast. Took {:?}",
        render_time
    );
}

