use livetree::render::RenderConfig;
use livetree::tree::{build_ignore_set, TreeConfig};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Default TreeConfig with standard ignore patterns.
pub fn default_tree_config() -> TreeConfig {
    TreeConfig {
        max_depth: None,
        show_hidden: false,
        dirs_only: false,
        follow_symlinks: false,
        ignore_patterns: build_ignore_set(&[]),
    }
}

/// RenderConfig with color disabled.
pub fn no_color_render_config(width: u16) -> RenderConfig {
    RenderConfig {
        use_color: false,
        terminal_width: width,
    }
}

/// RenderConfig with color enabled.
pub fn color_render_config(width: u16) -> RenderConfig {
    RenderConfig {
        use_color: true,
        terminal_width: width,
    }
}

/// Create a directory structure from a list of relative paths.
/// Paths ending with '/' create directories; others create empty files.
pub fn create_fixture(paths: &[&str]) -> TempDir {
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

/// Create a TreeEntry for testing purposes.
pub fn make_entry(
    name: &str,
    depth: usize,
    is_dir: bool,
    is_symlink: bool,
    is_last: bool,
    prefix: &str,
    error: Option<&str>,
) -> livetree::tree::TreeEntry {
    livetree::tree::TreeEntry {
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/test/{}", name)),
        depth,
        is_dir,
        is_symlink,
        symlink_target: None,
        is_last,
        prefix: prefix.to_string(),
        error: error.map(|s| s.to_string()),
    }
}

/// Extract plain text from a ratatui Line.
pub fn line_to_text(line: &ratatui::text::Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}
