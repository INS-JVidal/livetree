mod common;

use common::{color_render_config, make_entry, no_color_render_config};
use livetree::render::{
    help_bar_line, line_to_plain_text, status_bar_line, tree_to_lines, RenderConfig,
};
use livetree::tree::TreeEntry;
use ratatui::style::{Color, Modifier};
use std::collections::HashSet;
use std::path::PathBuf;

fn no_color_config() -> RenderConfig {
    no_color_render_config(120)
}

fn color_config() -> RenderConfig {
    color_render_config(120)
}

// --- Test 1: Plain file (no color) ---
#[test]
fn test_tree_to_lines_plain_file_no_color() {
    let entry = make_entry(
        "hello.txt",
        1,
        false,
        false,
        true,
        "\u{2514}\u{2500}\u{2500} ",
        None,
    );
    let config = no_color_config();
    let lines = tree_to_lines(&[entry], &config, &HashSet::new());
    assert_eq!(lines.len(), 1);
    let text = line_to_plain_text(&lines[0]);
    assert_eq!(text, "\u{2514}\u{2500}\u{2500} hello.txt");
}

// --- Test 2: Directory with color ---
#[test]
fn test_tree_to_lines_directory_with_color() {
    let entry = make_entry(
        "src",
        1,
        true,
        false,
        false,
        "\u{251c}\u{2500}\u{2500} ",
        None,
    );
    let config = color_config();
    let lines = tree_to_lines(&[entry], &config, &HashSet::new());
    assert_eq!(lines.len(), 1);

    // Check that directory name span has bold blue style
    let line = &lines[0];
    let name_span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "src")
        .unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Blue),
        "Directory should be blue"
    );
    assert!(
        name_span.style.add_modifier.contains(Modifier::BOLD),
        "Directory should be bold"
    );

    // Check prefix span is white
    let prefix_span = &line.spans[0];
    assert_eq!(
        prefix_span.style.fg,
        Some(Color::White),
        "Prefix should be white"
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
    let lines = tree_to_lines(&[entry], &config, &HashSet::new());
    let line = &lines[0];

    // Check that symlink name span has cyan style
    let name_span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "link.txt")
        .unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Cyan),
        "Symlink should be cyan"
    );

    // Check arrow target is present
    let text = line_to_plain_text(line);
    assert!(
        text.contains(" -> "),
        "Symlink should show arrow to target. Got: {:?}",
        text
    );
}

// --- Test 4: Entry with error ---
#[test]
fn test_tree_to_lines_error_with_color() {
    let entry = make_entry(
        "broken_dir",
        1,
        true,
        false,
        true,
        "\u{2514}\u{2500}\u{2500} ",
        Some("Permission denied"),
    );
    let config = color_config();
    let lines = tree_to_lines(&[entry], &config, &HashSet::new());
    let line = &lines[0];

    // Check that error span has red style
    let error_span = line
        .spans
        .iter()
        .find(|s| s.content.contains("Permission denied"))
        .unwrap();
    assert_eq!(error_span.style.fg, Some(Color::Red), "Error should be red");

    let text = line_to_plain_text(line);
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
        make_entry(
            "src",
            1,
            true,
            false,
            false,
            "\u{251c}\u{2500}\u{2500} ",
            None,
        ),
        make_entry(
            "main.rs",
            2,
            false,
            false,
            true,
            "\u{2502}   \u{2514}\u{2500}\u{2500} ",
            None,
        ),
        make_entry(
            "README.md",
            1,
            false,
            false,
            true,
            "\u{2514}\u{2500}\u{2500} ",
            None,
        ),
    ];
    let config = no_color_config();
    let lines = tree_to_lines(&entries, &config, &HashSet::new());

    assert_eq!(lines.len(), 3, "Should have 3 lines");
    let texts: Vec<String> = lines.iter().map(line_to_plain_text).collect();
    assert!(texts[0].contains("src"), "First line should contain 'src'");
    assert!(
        texts[1].contains("main.rs"),
        "Second line should contain 'main.rs'"
    );
    assert!(
        texts[2].contains("README.md"),
        "Third line should contain 'README.md'"
    );
}

// --- Test 6: tree_to_lines with empty tree ---
#[test]
fn test_tree_to_lines_empty() {
    let entries: Vec<TreeEntry> = Vec::new();
    let config = no_color_config();
    let lines = tree_to_lines(&entries, &config, &HashSet::new());
    assert_eq!(lines.len(), 0, "Empty tree should produce 0 lines");
}

// --- Test 7: status_bar_line with timestamp ---
#[test]
fn test_status_bar_line_with_timestamp() {
    let bar = status_bar_line("/home/user/project", "42 entries", Some("14:30:05"));
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
    let bar = status_bar_line("/tmp/test", "10 entries", None);
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
    let bar = status_bar_line("/tmp/test", "10 entries", None);
    let span = &bar.spans[0];
    assert_eq!(
        span.style.fg,
        Some(Color::White),
        "Status bar should have white text"
    );
    assert_eq!(
        span.style.bg,
        Some(Color::DarkGray),
        "Status bar should have dark gray background"
    );
}

// --- Test 10: Changed entries get cyan bold style ---
#[test]
fn test_changed_entry_gets_cyan_style() {
    let entry = make_entry("modified.txt", 1, false, false, true, "└── ", None);
    let config = color_config();
    let changed: HashSet<PathBuf> = [entry.path.clone()].into_iter().collect();
    let lines = tree_to_lines(&[entry], &config, &changed);
    let line = &lines[0];

    // Prefix should stay white (tree symbols don't change color)
    let prefix_span = &line.spans[0];
    assert_eq!(
        prefix_span.style.fg,
        Some(Color::White),
        "Changed entry prefix should stay white"
    );

    // Name should be cyan bold
    let name_span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "modified.txt")
        .unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Cyan),
        "Changed entry name should be cyan"
    );
    assert!(
        name_span.style.add_modifier.contains(Modifier::BOLD),
        "Changed entry name should be bold"
    );
}

// --- Test 11: Changed directory overrides blue with cyan ---
#[test]
fn test_changed_directory_gets_cyan_not_blue() {
    let entry = make_entry("src", 1, true, false, false, "├── ", None);
    let config = color_config();
    let changed: HashSet<PathBuf> = [entry.path.clone()].into_iter().collect();
    let lines = tree_to_lines(&[entry], &config, &changed);
    let line = &lines[0];

    let name_span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "src")
        .unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Rgb(64, 224, 208)),
        "Changed directory should be turquoise, not blue"
    );
}

// --- Test: help_bar_line contains expected keys ---
#[test]
fn test_help_bar_line_contains_keys() {
    let bar = help_bar_line();
    let text = line_to_plain_text(&bar);
    assert!(
        text.contains("q:"),
        "Help bar should contain 'q:'. Got: {:?}",
        text
    );
    assert!(
        text.contains("r:"),
        "Help bar should contain 'r:'. Got: {:?}",
        text
    );
    assert!(
        text.contains("↑↓"),
        "Help bar should contain '↑↓'. Got: {:?}",
        text
    );
    assert!(
        text.contains("PgUp/PgDn"),
        "Help bar should contain 'PgUp/PgDn'. Got: {:?}",
        text
    );
    assert!(
        text.contains("Home/End"),
        "Help bar should contain 'Home/End'. Got: {:?}",
        text
    );
}

// --- Test: help_bar_line has DarkGray style ---
#[test]
fn test_help_bar_line_has_style() {
    let bar = help_bar_line();
    let span = &bar.spans[0];
    assert_eq!(
        span.style.fg,
        Some(Color::DarkGray),
        "Help bar should have DarkGray text"
    );
}

// --- Test: Changed entry with use_color=false gets no highlight ---
#[test]
fn test_changed_entry_no_color_ignores_highlight() {
    let entry = make_entry("modified.txt", 1, false, false, true, "└── ", None);
    let config = no_color_config();
    let changed: HashSet<PathBuf> = [entry.path.clone()].into_iter().collect();
    let lines = tree_to_lines(&[entry], &config, &changed);
    let line = &lines[0];

    // With color disabled, all spans should have default (no) style
    for span in &line.spans {
        assert_eq!(
            span.style,
            ratatui::style::Style::default(),
            "With use_color=false, span '{}' should have default style",
            span.content
        );
    }
}

// --- Test 12: Unchanged entry is unaffected by changed_paths ---
#[test]
fn test_unchanged_entry_keeps_normal_style() {
    let entry = make_entry("src", 1, true, false, false, "├── ", None);
    let config = color_config();
    let changed: HashSet<PathBuf> = [PathBuf::from("/tmp/test/other.txt")].into_iter().collect();
    let lines = tree_to_lines(&[entry], &config, &changed);
    let line = &lines[0];

    let name_span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "src")
        .unwrap();
    assert_eq!(
        name_span.style.fg,
        Some(Color::Blue),
        "Unchanged directory should remain blue"
    );
}

#[test]
fn test_tree_to_lines_sanitizes_control_chars() {
    let entry = TreeEntry {
        name: "bad\u{001B}[31mname".to_string(),
        path: PathBuf::from("/tmp/bad"),
        depth: 1,
        is_dir: false,
        is_symlink: true,
        symlink_target: Some("line1\nline2".to_string()),
        is_last: true,
        prefix: "└── ".to_string(),
        error: None,
    };
    let config = no_color_config();
    let lines = tree_to_lines(&[entry], &config, &HashSet::new());
    let text = line_to_plain_text(&lines[0]);
    assert!(
        !text.contains('\u{001B}'),
        "Rendered text must not contain raw escape chars: {:?}",
        text
    );
    assert!(
        text.contains("\\x1B"),
        "Escape char should be rendered as visible escape sequence"
    );
    assert!(
        text.contains("line1\\nline2"),
        "Newlines should be sanitized in symlink targets"
    );
}

#[test]
fn test_status_bar_sanitizes_control_chars() {
    let bar = status_bar_line("/tmp/\u{001B}[2J", "10 entries", Some("12:00:00\tUTC"));
    let text = line_to_plain_text(&bar);
    assert!(!text.contains('\u{001B}'));
    assert!(text.contains("\\x1B"));
    assert!(text.contains("\\tUTC"));
}
