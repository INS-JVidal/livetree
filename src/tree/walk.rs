use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

use super::layout::compute_tree_structure;
use super::{TreeConfig, TreeEntry};

/// Raw entry data collected during filesystem traversal, before layout computation.
pub(super) type RawEntry = (
    usize,
    String,
    PathBuf,
    bool,
    bool,
    Option<String>,
    Option<String>,
);

const DEFAULT_IGNORES: &[&str] = &[".git", "node_modules", "__pycache__", ".DS_Store"];

/// Build a GlobSet from user patterns plus the default ignore list.
/// Invalid patterns are skipped and reported to stderr.
pub fn build_ignore_set(user_patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    let mut invalid = Vec::new();
    for pattern in DEFAULT_IGNORES {
        if let Ok(g) = Glob::new(pattern) {
            builder.add(g);
        }
    }
    for pattern in user_patterns {
        match Glob::new(pattern) {
            Ok(g) => {
                builder.add(g);
            }
            Err(_) => {
                invalid.push(pattern.clone());
            }
        }
    }
    if !invalid.is_empty() {
        eprintln!(
            "livetree: invalid ignore pattern(s), skipped: {:?}",
            invalid
        );
    }
    builder.build().unwrap_or_else(|e| {
        eprintln!("livetree: failed to build ignore set: {}", e);
        GlobSet::empty()
    })
}

/// Build the tree from a root path.
pub fn build_tree(root: &Path, config: &TreeConfig) -> Vec<TreeEntry> {
    let mut walker = WalkDir::new(root)
        .follow_links(config.follow_symlinks)
        .sort_by(sort_cmp);

    if let Some(max_depth) = config.max_depth {
        walker = walker.max_depth(max_depth);
    }

    // Collect valid entries, using filter_entry to prevent descending
    // into hidden/ignored directories (not just skipping their display).
    let mut raw_entries: Vec<RawEntry> = Vec::new();

    let show_hidden = config.show_hidden;
    let ignore_patterns = config.ignore_patterns.clone();
    let root = root.to_path_buf();
    let iter = walker.into_iter().filter_entry(move |entry| {
        let name = entry.file_name().to_string_lossy();
        // Always allow root
        if entry.depth() == 0 {
            return true;
        }
        // Filter hidden entries (prevents descending into .git, etc.)
        if !show_hidden && name.starts_with('.') {
            return false;
        }
        // Filter ignored patterns: match path relative to root so e.g. "target/**" works
        let path_to_match = entry
            .path()
            .strip_prefix(&root)
            .unwrap_or_else(|_| entry.path());
        if ignore_patterns.is_match(path_to_match) {
            return false;
        }
        true
    });

    for entry_result in iter {
        match entry_result {
            Ok(entry) => {
                let depth = entry.depth();
                // Skip root itself
                if depth == 0 {
                    continue;
                }

                let file_name = entry.file_name().to_string_lossy().to_string();

                let is_dir = entry.file_type().is_dir();

                // Skip files if --dirs-only
                if config.dirs_only && !is_dir {
                    continue;
                }

                let is_symlink = entry.path_is_symlink();
                let path = entry.path().to_path_buf();
                let symlink_target = if is_symlink {
                    Some(
                        std::fs::read_link(&path)
                            .map(|t| t.to_string_lossy().to_string())
                            .unwrap_or_else(|_| "?".to_string()),
                    )
                } else {
                    None
                };

                raw_entries.push((
                    depth,
                    file_name,
                    path,
                    is_dir,
                    is_symlink,
                    symlink_target,
                    None,
                ));
            }
            Err(e) => {
                // walkdir error — extract what we can
                let depth = e.depth();
                if depth == 0 {
                    continue;
                }
                let path = e.path().map(|p| p.to_path_buf()).unwrap_or_default();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "???".to_string());
                let error_msg = if let Some(io_err) = e.io_error() {
                    io_err.to_string()
                } else {
                    e.to_string()
                };
                raw_entries.push((depth, name, path, true, false, None, Some(error_msg)));
            }
        }
    }

    // Now compute is_last and prefixes
    compute_tree_structure(&raw_entries)
}

/// Comparison function for walkdir sorting.
/// Directories first, then case-insensitive alpha, dotfiles last.
fn sort_cmp(a: &DirEntry, b: &DirEntry) -> std::cmp::Ordering {
    let a_is_dir = a.file_type().is_dir();
    let b_is_dir = b.file_type().is_dir();

    // Directories before files
    if a_is_dir != b_is_dir {
        return if a_is_dir {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        };
    }

    let a_name = a.file_name().to_string_lossy().to_string();
    let b_name = b.file_name().to_string_lossy().to_string();

    let a_dot = a_name.starts_with('.');
    let b_dot = b_name.starts_with('.');

    // Dotfiles last
    if a_dot != b_dot {
        return if a_dot {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Less
        };
    }

    // Case-insensitive alphabetical
    a_name.to_lowercase().cmp(&b_name.to_lowercase())
}
