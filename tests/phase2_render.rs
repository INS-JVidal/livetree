mod common;

use common::{color_render_config, make_entry, no_color_render_config, strip_ansi};
use livetree::render::{format_entry, format_status_bar, render_tree, RenderConfig};
use livetree::tree::TreeEntry;

fn no_color_config() -> RenderConfig {
    no_color_render_config(120)
}

fn color_config() -> RenderConfig {
    color_render_config(120)
}

// --- Test 1: Plain file (no color) ---
#[test]
fn test_format_entry_plain_file_no_color() {
    let entry = make_entry("hello.txt", 1, false, false, true, "\u{2514}\u{2500}\u{2500} ", None);
    let config = no_color_config();
    let line = format_entry(&entry, &config);
    assert_eq!(line, "\u{2514}\u{2500}\u{2500} hello.txt");
}

// --- Test 2: Directory with color ---
#[test]
fn test_format_entry_directory_with_color() {
    let entry = make_entry("src", 1, true, false, false, "\u{251c}\u{2500}\u{2500} ", None);
    let config = color_config();
    let line = format_entry(&entry, &config);
    // Should contain bold blue for directory name
    assert!(
        line.contains("\x1b[1;34m"),
        "Directory should be bold blue. Got: {:?}",
        line
    );
    assert!(line.contains("src"), "Should contain the directory name");
    // Should contain dim for prefix
    assert!(
        line.contains("\x1b[2m"),
        "Prefix should be dim. Got: {:?}",
        line
    );
}

// --- Test 3: Symlink with color ---
#[test]
#[cfg(unix)]
fn test_format_entry_symlink_with_color() {
    // Create a real symlink so read_link works
    let tmp = tempfile::TempDir::new().unwrap();
    let target_path = tmp.path().join("target.txt");
    std::fs::write(&target_path, "content").unwrap();
    let link_path = tmp.path().join("link.txt");
    std::os::unix::fs::symlink(&target_path, &link_path).unwrap();

    let entry = TreeEntry {
        name: "link.txt".to_string(),
        path: link_path.clone(),
        depth: 1,
        is_dir: false,
        is_symlink: true,
        symlink_target: Some(
            std::fs::read_link(&link_path)
                .map(|t| t.to_string_lossy().to_string())
                .unwrap_or_else(|_| "?".to_string()),
        ),
        is_last: true,
        prefix: "\u{2514}\u{2500}\u{2500} ".to_string(),
        error: None,
    };
    let config = color_config();
    let line = format_entry(&entry, &config);
    // Should contain cyan for symlink
    assert!(
        line.contains("\x1b[36m"),
        "Symlink should be cyan. Got: {:?}",
        line
    );
    // Should contain the arrow and target
    assert!(
        line.contains(" -> "),
        "Symlink should show arrow to target. Got: {:?}",
        line
    );
}

// --- Test 4: Entry with error ---
#[test]
fn test_format_entry_error_with_color() {
    let entry = make_entry("broken_dir", 1, true, false, true, "\u{2514}\u{2500}\u{2500} ", Some("Permission denied"));
    let config = color_config();
    let line = format_entry(&entry, &config);
    // Should contain red for error
    assert!(
        line.contains("\x1b[31m"),
        "Error should be red. Got: {:?}",
        line
    );
    // Should contain the error message
    assert!(
        line.contains("Permission denied"),
        "Should contain error text. Got: {:?}",
        line
    );
}

// --- Test 5: Long filename truncation ---
#[test]
fn test_format_entry_long_name_truncation() {
    let long_name = "a".repeat(200);
    let entry = make_entry(&long_name, 1, false, false, true, "\u{2514}\u{2500}\u{2500} ", None);
    let config = RenderConfig {
        use_color: false,
        terminal_width: 40,
    };
    let line = format_entry(&entry, &config);

    // Strip ANSI codes for width measurement
    let plain = strip_ansi(&line);
    let display_width = unicode_width::UnicodeWidthStr::width(plain.as_str());
    assert!(
        display_width <= 40,
        "Line display width should be <= 40, got {}. Line: {:?}",
        display_width,
        line
    );
    assert!(
        line.contains("\u{2026}"),
        "Truncated line should end with ellipsis. Got: {:?}",
        line
    );
}

// --- Test 6: render_tree with 3 entries ---
#[test]
fn test_render_tree_normal() {
    let entries = vec![
        make_entry("src", 1, true, false, false, "\u{251c}\u{2500}\u{2500} ", None),
        make_entry("main.rs", 2, false, false, true, "\u{2502}   \u{2514}\u{2500}\u{2500} ", None),
        make_entry("README.md", 1, false, false, true, "\u{2514}\u{2500}\u{2500} ", None),
    ];
    let config = no_color_config();
    let mut output = Vec::new();
    let count = render_tree(&mut output, &entries, &config).unwrap();

    assert_eq!(count, 3, "Should have written 3 lines");
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("src"), "Output should contain 'src'");
    assert!(text.contains("main.rs"), "Output should contain 'main.rs'");
    assert!(text.contains("README.md"), "Output should contain 'README.md'");
}

// --- Test 7: render_tree with empty tree ---
#[test]
fn test_render_tree_empty() {
    let entries: Vec<TreeEntry> = Vec::new();
    let config = no_color_config();
    let mut output = Vec::new();
    let count = render_tree(&mut output, &entries, &config).unwrap();

    assert_eq!(count, 0, "Empty tree should produce 0 lines");
    assert!(output.is_empty(), "Output should be empty for empty tree");
}

// --- Test 8: format_status_bar with timestamp ---
#[test]
fn test_format_status_bar_with_timestamp() {
    let bar = format_status_bar("/home/user/project", "42 entries", Some("14:30:05"), 80);
    assert!(
        bar.contains("Watching: /home/user/project"),
        "Should contain watched path. Got: {:?}",
        bar
    );
    assert!(
        bar.contains("42 entries"),
        "Should contain entry count. Got: {:?}",
        bar
    );
    assert!(
        bar.contains("Last change: 14:30:05"),
        "Should contain timestamp. Got: {:?}",
        bar
    );
}

// --- Test 9: format_status_bar with no change ---
#[test]
fn test_format_status_bar_no_change() {
    let bar = format_status_bar("/tmp/test", "10 entries", None, 80);
    assert!(
        bar.contains("No changes yet"),
        "Should show 'No changes yet'. Got: {:?}",
        bar
    );
}

// --- Test 10: format_status_bar with very long path ---
#[test]
fn test_format_status_bar_long_path() {
    let long_path = "/home/user/".to_string() + &"very_long_directory_name/".repeat(20);
    let bar = format_status_bar(&long_path, "100 entries", Some("12:00:00"), 60);
    let plain = strip_ansi(&bar);
    let display_width = unicode_width::UnicodeWidthStr::width(plain.as_str());
    assert!(
        display_width <= 60,
        "Status bar should not exceed terminal width. Got width={}, bar: {:?}",
        display_width,
        bar
    );
}

