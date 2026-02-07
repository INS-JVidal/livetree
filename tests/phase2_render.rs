mod common;

use common::{color_render_config, line_to_text, make_entry, no_color_render_config};
use livetree::render::{line_to_plain_text, status_bar_line, tree_to_lines, RenderConfig};
use livetree::tree::TreeEntry;
use ratatui::style::{Color, Modifier};

fn no_color_config() -> RenderConfig {
    no_color_render_config(120)
}

fn color_config() -> RenderConfig {
    color_render_config(120)
}

// --- Test 1: Plain file (no color) ---
#[test]
fn test_tree_to_lines_plain_file_no_color() {
    let entry = make_entry("hello.txt", 1, false, false, true, "\u{2514}\u{2500}\u{2500} ", None);
    let config = no_color_config();
    let lines = tree_to_lines(&[entry], &config);
    assert_eq!(lines.len(), 1);
    let text = line_to_text(&lines[0]);
    assert_eq!(text, "\u{2514}\u{2500}\u{2500} hello.txt");
}

// --- Test 2: Directory with color ---
#[test]
fn test_tree_to_lines_directory_with_color() {
    let entry = make_entry("src", 1, true, false, false, "\u{251c}\u{2500}\u{2500} ", None);
    let config = color_config();
    let lines = tree_to_lines(&[entry], &config);
    assert_eq!(lines.len(), 1);

    // Check that directory name span has bold blue style
    let line = &lines[0];
    let name_span = line.spans.iter().find(|s| s.content.as_ref() == "src").unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Blue),
        "Directory should be blue"
    );
    assert!(
        name_span.style.add_modifier.contains(Modifier::BOLD),
        "Directory should be bold"
    );

    // Check prefix span is dim
    let prefix_span = &line.spans[0];
    assert!(
        prefix_span.style.add_modifier.contains(Modifier::DIM),
        "Prefix should be dim"
    );
}

// --- Test 3: Symlink with color ---
#[test]
#[cfg(unix)]
fn test_tree_to_lines_symlink_with_color() {
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
    let lines = tree_to_lines(&[entry], &config);
    let line = &lines[0];

    // Check that symlink name span has cyan style
    let name_span = line.spans.iter().find(|s| s.content.as_ref() == "link.txt").unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Cyan),
        "Symlink should be cyan"
    );

    // Check arrow target is present
    let text = line_to_text(line);
    assert!(
        text.contains(" -> "),
        "Symlink should show arrow to target. Got: {:?}",
        text
    );
}

// --- Test 4: Entry with error ---
#[test]
fn test_tree_to_lines_error_with_color() {
    let entry = make_entry("broken_dir", 1, true, false, true, "\u{2514}\u{2500}\u{2500} ", Some("Permission denied"));
    let config = color_config();
    let lines = tree_to_lines(&[entry], &config);
    let line = &lines[0];

    // Check that error span has red style
    let error_span = line.spans.iter().find(|s| s.content.contains("Permission denied")).unwrap();
    assert_eq!(
        error_span.style.fg,
        Some(Color::Red),
        "Error should be red"
    );

    let text = line_to_text(line);
    assert!(
        text.contains("Permission denied"),
        "Should contain error text. Got: {:?}",
        text
    );
}

// --- Test 5: tree_to_lines with 3 entries ---
#[test]
fn test_tree_to_lines_normal() {
    let entries = vec![
        make_entry("src", 1, true, false, false, "\u{251c}\u{2500}\u{2500} ", None),
        make_entry("main.rs", 2, false, false, true, "\u{2502}   \u{2514}\u{2500}\u{2500} ", None),
        make_entry("README.md", 1, false, false, true, "\u{2514}\u{2500}\u{2500} ", None),
    ];
    let config = no_color_config();
    let lines = tree_to_lines(&entries, &config);

    assert_eq!(lines.len(), 3, "Should have 3 lines");
    let texts: Vec<String> = lines.iter().map(line_to_text).collect();
    assert!(texts[0].contains("src"), "First line should contain 'src'");
    assert!(texts[1].contains("main.rs"), "Second line should contain 'main.rs'");
    assert!(texts[2].contains("README.md"), "Third line should contain 'README.md'");
}

// --- Test 6: tree_to_lines with empty tree ---
#[test]
fn test_tree_to_lines_empty() {
    let entries: Vec<TreeEntry> = Vec::new();
    let config = no_color_config();
    let lines = tree_to_lines(&entries, &config);
    assert_eq!(lines.len(), 0, "Empty tree should produce 0 lines");
}

// --- Test 7: status_bar_line with timestamp ---
#[test]
fn test_status_bar_line_with_timestamp() {
    let bar = status_bar_line("/home/user/project", "42 entries", Some("14:30:05"), 80);
    let text = line_to_plain_text(&bar);
    assert!(
        text.contains("Watching: /home/user/project"),
        "Should contain watched path. Got: {:?}",
        text
    );
    assert!(
        text.contains("42 entries"),
        "Should contain entry count. Got: {:?}",
        text
    );
    assert!(
        text.contains("Last change: 14:30:05"),
        "Should contain timestamp. Got: {:?}",
        text
    );
}

// --- Test 8: status_bar_line with no change ---
#[test]
fn test_status_bar_line_no_change() {
    let bar = status_bar_line("/tmp/test", "10 entries", None, 80);
    let text = line_to_plain_text(&bar);
    assert!(
        text.contains("No changes yet"),
        "Should show 'No changes yet'. Got: {:?}",
        text
    );
}

// --- Test 9: status_bar_line has styling ---
#[test]
fn test_status_bar_line_has_style() {
    let bar = status_bar_line("/tmp/test", "10 entries", None, 80);
    let span = &bar.spans[0];
    assert_eq!(span.style.fg, Some(Color::White), "Status bar should have white text");
    assert_eq!(span.style.bg, Some(Color::DarkGray), "Status bar should have dark gray background");
}
