//! Tree building, filtering, sorting, and layout computation.

mod layout;
pub(crate) mod walk;

use globset::GlobSet;
use std::path::PathBuf;

pub use walk::{build_ignore_set, build_tree};

/// A single entry in the rendered directory tree.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeEntry {
    /// Display name (filename component only).
    pub name: String,
    /// Full filesystem path.
    pub path: PathBuf,
    /// Nesting depth (1 = direct child of root).
    pub depth: usize,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// Whether this entry is a symbolic link.
    pub is_symlink: bool,
    /// Resolved symlink target path, if this entry is a symlink.
    pub symlink_target: Option<String>,
    /// Whether this is the last sibling in its parent group.
    pub is_last: bool,
    /// Pre-computed box-drawing prefix string for tree display.
    pub prefix: String,
    /// Error message if the entry could not be read (e.g. permission denied).
    pub error: Option<String>,
}

/// Configuration for tree building.
pub struct TreeConfig {
    /// Maximum traversal depth (`None` for unlimited).
    pub max_depth: Option<usize>,
    /// Whether to include hidden files (dotfiles).
    pub show_hidden: bool,
    /// Whether to show only directories.
    pub dirs_only: bool,
    /// Whether to follow symbolic links during traversal.
    pub follow_symlinks: bool,
    /// Glob patterns for entries to exclude.
    pub ignore_patterns: GlobSet,
}
