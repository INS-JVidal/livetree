#[test]
fn test_terminal_size_returns_nonzero() {
    let (w, h) = livetree::terminal::terminal_size();
    assert!(w > 0, "Width should be > 0 (fallback is 80)");
    assert!(h > 0, "Height should be > 0 (fallback is 24)");
}
