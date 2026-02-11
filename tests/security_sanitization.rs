use livetree::render::{line_to_plain_text, status_bar_line, tree_to_lines, RenderConfig};
use livetree::tree::TreeEntry;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_terminal_control_chars_are_sanitized_in_rendered_output() {
    let entry = TreeEntry {
        name: "evil\u{001B}[31mname\twith\ncontrols".to_string(),
        path: PathBuf::from("/tmp/evil"),
        depth: 1,
        is_dir: false,
        is_symlink: true,
        symlink_target: Some("target\r\u{001B}[2J".to_string()),
        is_last: true,
        prefix: "└── ".to_string(),
        error: Some("bad\tinput\nvalue\r".to_string()),
    };

    let lines = tree_to_lines(
        &[entry],
        &RenderConfig {
            use_color: false,
            terminal_width: 120,
        },
        &HashSet::new(),
    );
    assert_eq!(lines.len(), 1);
    let rendered = line_to_plain_text(&lines[0]);

    // No raw ESC/control chars should remain in rendered tree lines.
    assert!(!rendered.contains('\u{001B}'));
    assert!(!rendered.contains('\n'));
    assert!(!rendered.contains('\r'));
    assert!(!rendered.contains('\t'));

    // Escaped forms should be visible for debugging/auditing.
    assert!(rendered.contains("\\x1B"));
    assert!(rendered.contains("\\n"));
    assert!(rendered.contains("\\r"));
    assert!(rendered.contains("\\t"));

    let status = status_bar_line(
        "/tmp/\u{001B}[2Jpath",
        "10 entries\twith\nnoise",
        Some("12:00:00\rZ"),
    );
    let status_text = line_to_plain_text(&status);
    assert!(!status_text.contains('\u{001B}'));
    assert!(!status_text.contains('\n'));
    assert!(!status_text.contains('\r'));
    assert!(!status_text.contains('\t'));
    assert!(status_text.contains("\\x1B"));
    assert!(status_text.contains("\\n"));
    assert!(status_text.contains("\\r"));
    assert!(status_text.contains("\\t"));
}
