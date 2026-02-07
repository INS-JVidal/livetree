use livetree::tree::{build_ignore_set, build_ignore_set_no_defaults, build_tree, TreeConfig, TreeEntry};
use std::fs;
use tempfile::TempDir;

/// Helper: create a directory structure from a list of relative paths.
/// Paths ending with '/' create directories; others create empty files.
fn create_fixture(paths: &[&str]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    for p in paths {
        let full = tmp.path().join(p);
        if p.ends_with('/') {
            fs::create_dir_all(&full).unwrap();
        } else {
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full, "").unwrap();
        }
    }
    tmp
}

fn default_config() -> TreeConfig {
    TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    }
}

// --- Sorting ---

#[test]
fn test_directories_before_files() {
    let tmp = create_fixture(&["src/", "README.md", "build/", "main.rs"]);
    let entries = build_tree(tmp.path(), &default_config());

    let top: Vec<&TreeEntry> = entries.iter().filter(|e| e.depth == 1).collect();

    // Find boundary: all dirs should come before all files
    let first_file_idx = top.iter().position(|e| !e.is_dir);
    let last_dir_idx = top.iter().rposition(|e| e.is_dir);

    if let (Some(first_file), Some(last_dir)) = (first_file_idx, last_dir_idx) {
        assert!(
            last_dir < first_file,
            "All directories should sort before files. Got: {:?}",
            top.iter().map(|e| &e.name).collect::<Vec<_>>()
        );
    }
}

#[test]
fn test_case_insensitive_alpha_sort() {
    let tmp = create_fixture(&["Banana.txt", "apple.txt", "Cherry.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let names: Vec<&str> = entries
        .iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(names, vec!["apple.txt", "Banana.txt", "Cherry.txt"]);
}

#[test]
fn test_dotfiles_hidden_by_default() {
    let tmp = create_fixture(&[".hidden", "visible.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let names: Vec<&str> = entries
        .iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&".hidden"));
    assert!(names.contains(&"visible.txt"));
}

#[test]
fn test_dotfiles_shown_with_all_flag() {
    let tmp = create_fixture(&[".hidden", "visible.txt"]);
    let mut cfg = default_config();
    cfg.show_hidden = true;
    // Use no-defaults to avoid .hidden being caught by default ignores
    cfg.ignore_patterns = build_ignore_set_no_defaults(&[]);
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries
        .iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert!(names.contains(&".hidden"));
    assert!(names.contains(&"visible.txt"));
    // Dotfiles should sort after non-dotfiles
    let hidden_idx = names.iter().position(|n| *n == ".hidden").unwrap();
    let visible_idx = names.iter().position(|n| *n == "visible.txt").unwrap();
    assert!(
        hidden_idx > visible_idx,
        "Dotfiles should sort after regular files. Got: {:?}",
        names
    );
}

// --- Depth Limiting ---

#[test]
fn test_depth_limit_1() {
    let tmp = create_fixture(&["a/", "a/b/", "a/b/c.txt", "a/d.txt", "e.txt"]);
    let mut cfg = default_config();
    cfg.max_depth = Some(1);
    let entries = build_tree(tmp.path(), &cfg);
    assert!(
        entries.iter().all(|e| e.depth <= 1),
        "No entry should exceed depth 1"
    );
}

#[test]
fn test_depth_limit_2() {
    let tmp = create_fixture(&["a/", "a/b/", "a/b/deep.txt", "a/top.txt"]);
    let mut cfg = default_config();
    cfg.max_depth = Some(2);
    let entries = build_tree(tmp.path(), &cfg);
    assert!(entries.iter().all(|e| e.depth <= 2));
    assert!(entries.iter().any(|e| e.depth == 2));
}

// --- Ignore Patterns ---

#[test]
fn test_default_ignores() {
    let tmp = create_fixture(&[
        ".git/",
        ".git/config",
        "node_modules/",
        "node_modules/pkg/",
        "__pycache__/",
        "src/",
        "src/main.rs",
    ]);
    let mut cfg = default_config();
    cfg.show_hidden = true;
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(!names.contains(&".git"), "Should ignore .git");
    assert!(
        !names.contains(&"node_modules"),
        "Should ignore node_modules"
    );
    assert!(!names.contains(&"__pycache__"), "Should ignore __pycache__");
    assert!(names.contains(&"src"), "Should keep src");
}

#[test]
fn test_custom_ignore_pattern() {
    let tmp = create_fixture(&["debug.log", "app.log", "main.rs", "lib.rs"]);
    let mut cfg = default_config();
    cfg.ignore_patterns = build_ignore_set(&["*.log".to_string()]);
    let entries = build_tree(tmp.path(), &cfg);
    let names: Vec<&str> = entries
        .iter()
        .filter(|e| e.depth == 1)
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&"debug.log"));
    assert!(!names.contains(&"app.log"));
    assert!(names.contains(&"main.rs"));
}

// --- Dirs Only ---

#[test]
fn test_dirs_only() {
    let tmp = create_fixture(&["src/", "src/main.rs", "tests/", "README.md"]);
    let mut cfg = default_config();
    cfg.dirs_only = true;
    let entries = build_tree(tmp.path(), &cfg);
    assert!(
        entries.iter().filter(|e| e.depth >= 1).all(|e| e.is_dir),
        "All entries should be directories when dirs_only is set"
    );
}

// --- Prefix Computation ---

#[test]
fn test_prefix_simple_tree() {
    let tmp = create_fixture(&["a/", "a/deep.txt", "b.txt"]);
    let entries = build_tree(tmp.path(), &default_config());

    let a_entry = entries.iter().find(|e| e.name == "a").unwrap();
    assert!(
        a_entry.prefix.contains('\u{251c}'),
        "Dir 'a' should use \u{251c}\u{2500}\u{2500} (got: {:?})",
        a_entry.prefix
    );

    let b_entry = entries.iter().find(|e| e.name == "b.txt").unwrap();
    assert!(
        b_entry.prefix.contains('\u{2514}'),
        "'b.txt' should use \u{2514}\u{2500}\u{2500} (got: {:?})",
        b_entry.prefix
    );

    let deep = entries.iter().find(|e| e.name == "deep.txt").unwrap();
    assert!(
        deep.prefix.contains('\u{2514}'),
        "Nested last child should use \u{2514}\u{2500}\u{2500}"
    );
}

#[test]
fn test_prefix_deeply_nested() {
    let tmp = create_fixture(&["a/", "a/b/", "a/b/c/", "a/b/c/d.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let d = entries.iter().find(|e| e.name == "d.txt").unwrap();
    assert!(
        d.depth >= 3,
        "d.txt should be deeply nested, got depth={}",
        d.depth
    );
    // Prefix should have continuation lines
    assert!(
        d.prefix.len() > 8,
        "Deep prefix should be long: {:?}",
        d.prefix
    );
}

// --- is_last Flag ---

#[test]
fn test_is_last_flag() {
    let tmp = create_fixture(&["alpha.txt", "beta.txt", "gamma.txt"]);
    let entries = build_tree(tmp.path(), &default_config());
    let top: Vec<&TreeEntry> = entries.iter().filter(|e| e.depth == 1).collect();
    for (i, entry) in top.iter().enumerate() {
        if i == top.len() - 1 {
            assert!(
                entry.is_last,
                "Last entry '{}' should have is_last=true",
                entry.name
            );
        } else {
            assert!(
                !entry.is_last,
                "Entry '{}' should have is_last=false",
                entry.name
            );
        }
    }
}

// --- Empty Directory ---

#[test]
fn test_empty_directory() {
    let tmp = TempDir::new().unwrap();
    let entries = build_tree(tmp.path(), &default_config());
    assert!(
        entries.iter().filter(|e| e.depth >= 1).count() == 0,
        "Empty directory should produce no child entries"
    );
}

// --- Symlinks ---

#[test]
#[cfg(unix)]
fn test_symlink_detected() {
    let tmp = create_fixture(&["target.txt"]);
    std::os::unix::fs::symlink(
        tmp.path().join("target.txt"),
        tmp.path().join("link.txt"),
    )
    .unwrap();
    let entries = build_tree(tmp.path(), &default_config());
    let link = entries.iter().find(|e| e.name == "link.txt");
    assert!(link.is_some(), "Symlink should appear in tree");
    assert!(
        link.unwrap().is_symlink,
        "Symlink should be flagged as is_symlink"
    );
}
