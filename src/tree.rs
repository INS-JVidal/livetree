use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub is_last: bool,
    pub prefix: String,
    pub error: Option<String>,
}

pub struct TreeConfig {
    pub max_depth: Option<usize>,
    pub show_hidden: bool,
    pub dirs_only: bool,
    pub follow_symlinks: bool,
    pub ignore_patterns: GlobSet,
}

const DEFAULT_IGNORES: &[&str] = &[".git", "node_modules", "__pycache__", ".DS_Store"];

/// Build a GlobSet from user patterns plus the default ignore list.
pub fn build_ignore_set(user_patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in DEFAULT_IGNORES {
        if let Ok(g) = Glob::new(pattern) {
            builder.add(g);
        }
    }
    for pattern in user_patterns {
        if let Ok(g) = Glob::new(pattern) {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Build a GlobSet from only user patterns (no defaults).
pub fn build_ignore_set_no_defaults(user_patterns: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in user_patterns {
        if let Ok(g) = Glob::new(pattern) {
            builder.add(g);
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// Build the tree from a root path.
pub fn build_tree(root: &Path, config: &TreeConfig) -> Vec<TreeEntry> {
    let mut walker = WalkDir::new(root)
        .follow_links(config.follow_symlinks)
        .sort_by(|a, b| sort_cmp(a, b));

    if let Some(max_depth) = config.max_depth {
        walker = walker.max_depth(max_depth);
    }

    // Collect valid entries, using filter_entry to prevent descending
    // into hidden/ignored directories (not just skipping their display).
    let mut raw_entries: Vec<(usize, String, PathBuf, bool, bool, Option<String>)> = Vec::new();

    let show_hidden = config.show_hidden;
    let ignore_patterns = config.ignore_patterns.clone();
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
        // Filter ignored patterns (prevents descending into node_modules, etc.)
        if ignore_patterns.is_match(name.as_ref()) {
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

                raw_entries.push((depth, file_name, path, is_dir, is_symlink, None));
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
                raw_entries.push((depth, name, path, true, false, Some(error_msg)));
            }
        }
    }

    // Now compute is_last and prefixes
    compute_tree_structure(&raw_entries)
}

/// Compute is_last flags and prefix strings for all entries.
fn compute_tree_structure(
    raw: &[(usize, String, PathBuf, bool, bool, Option<String>)],
) -> Vec<TreeEntry> {
    let len = raw.len();
    let mut entries = Vec::with_capacity(len);

    for (i, (depth, name, path, is_dir, is_symlink, error)) in raw.iter().enumerate() {
        // An entry is_last if no subsequent sibling exists at the same depth
        // under the same parent. A sibling is the next entry at the same depth
        // before we see an entry at a lesser depth.
        let is_last = is_last_sibling(raw, i);

        entries.push(TreeEntry {
            name: name.clone(),
            path: path.clone(),
            depth: *depth,
            is_dir: *is_dir,
            is_symlink: *is_symlink,
            is_last,
            prefix: String::new(), // computed below
            error: error.clone(),
        });
    }

    // Compute prefixes using an ancestor_is_last stack
    compute_prefixes(&mut entries);

    entries
}

/// Determine if entry at index `i` is the last sibling in its parent group.
fn is_last_sibling(
    raw: &[(usize, String, PathBuf, bool, bool, Option<String>)],
    i: usize,
) -> bool {
    let depth = raw[i].0;
    // Look ahead for next entry at the same or lesser depth
    for j in (i + 1)..raw.len() {
        let next_depth = raw[j].0;
        if next_depth == depth {
            return false; // there's another sibling
        }
        if next_depth < depth {
            return true; // parent's scope ended, we were last
        }
        // next_depth > depth means it's a child of us, keep looking
    }
    // Reached end of list — we are last
    true
}

/// Compute prefix strings for all entries.
/// Uses the is_last flag of ancestors to determine continuation lines.
fn compute_prefixes(entries: &mut [TreeEntry]) {
    // Track is_last for each depth level
    // ancestor_is_last[d] = true means the ancestor at depth d was the last sibling
    let mut ancestor_is_last: Vec<bool> = Vec::new();

    for entry in entries.iter_mut() {
        let depth = entry.depth;

        // Ensure ancestor stack is the right size
        while ancestor_is_last.len() < depth {
            ancestor_is_last.push(false);
        }
        ancestor_is_last.truncate(depth);

        // Build prefix from ancestors
        let mut prefix = String::new();
        for d in 1..depth {
            if d <= ancestor_is_last.len() && ancestor_is_last[d - 1] {
                prefix.push_str("    ");
            } else {
                prefix.push_str("\u{2502}   "); // │
            }
        }

        // Add the connector for this entry
        if depth > 0 {
            if entry.is_last {
                prefix.push_str("\u{2514}\u{2500}\u{2500} "); // └──
            } else {
                prefix.push_str("\u{251c}\u{2500}\u{2500} "); // ├──
            }
        }

        entry.prefix = prefix;

        // Record whether this entry is_last at its depth for children
        if ancestor_is_last.len() < depth {
            ancestor_is_last.push(entry.is_last);
        } else if depth > 0 {
            ancestor_is_last[depth - 1] = entry.is_last;
        }
    }
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
