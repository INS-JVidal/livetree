use livetree::terminal::render_frame;

/// Helper: extract all clear-line CSI sequences (\x1b[2K) from output.
fn count_clear_sequences(data: &[u8]) -> usize {
    let s = String::from_utf8_lossy(data);
    // crossterm emits CSI 2 K for ClearCurrentLine
    // The raw bytes are: \x1b [ 2 K
    s.matches("\x1b[2K").count()
}

/// Helper: check output contains cursor-home sequence.
fn has_cursor_home(data: &[u8]) -> bool {
    let s = String::from_utf8_lossy(data);
    // CSI H or CSI 1;1H — crossterm emits \x1b[1;1H
    s.contains("\x1b[1;1H") || s.contains("\x1b[H")
}

#[test]
fn test_render_frame_writes_cursor_home() {
    let mut buf = Vec::new();
    let lines = vec!["├── file.txt".to_string()];

    render_frame(&mut buf, &lines, 0, 1000).unwrap();

    assert!(
        has_cursor_home(&buf),
        "Output should start with cursor-home escape. Got: {:?}",
        String::from_utf8_lossy(&buf)
    );
}

#[test]
fn test_render_frame_clears_leftover_lines() {
    let mut buf = Vec::new();
    let lines = vec!["line1".to_string()];

    // Previous frame had 5 lines, current has 1 → should clear current + 4 leftovers
    render_frame(&mut buf, &lines, 5, 1000).unwrap();

    let clear_count = count_clear_sequences(&buf);
    assert!(
        clear_count >= 5,
        "Should clear at least 5 lines (1 current + 4 leftover). Got {} clears",
        clear_count
    );
}

#[test]
fn test_render_frame_returns_line_count() {
    let mut buf = Vec::new();
    let lines = vec![
        "line1".to_string(),
        "line2".to_string(),
        "line3".to_string(),
    ];
    let count = render_frame(&mut buf, &lines, 0, 1000).unwrap();
    assert_eq!(count, 3);
}

#[test]
fn test_render_frame_empty_clears_previous() {
    let mut buf = Vec::new();
    let count = render_frame(&mut buf, &[], 3, 1000).unwrap();
    assert_eq!(count, 0);

    let clear_count = count_clear_sequences(&buf);
    assert!(
        clear_count >= 3,
        "Should clear 3 leftover lines. Got {} clears",
        clear_count
    );
}

#[test]
fn test_render_frame_content_present() {
    let mut buf = Vec::new();
    let lines = vec![
        "├── src".to_string(),
        "│   └── main.rs".to_string(),
        "└── README.md".to_string(),
    ];
    render_frame(&mut buf, &lines, 0, 1000).unwrap();

    let output = String::from_utf8_lossy(&buf);
    assert!(output.contains("src"), "Output should contain 'src'");
    assert!(output.contains("main.rs"), "Output should contain 'main.rs'");
    assert!(
        output.contains("README.md"),
        "Output should contain 'README.md'"
    );
}

#[test]
fn test_terminal_size_returns_nonzero() {
    let (w, h) = livetree::terminal::terminal_size();
    assert!(w > 0, "Width should be > 0 (fallback is 80)");
    assert!(h > 0, "Height should be > 0 (fallback is 24)");
}
