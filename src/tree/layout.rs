use super::walk::RawEntry;
use super::TreeEntry;

/// Compute is_last flags and prefix strings for all entries.
pub(super) fn compute_tree_structure(raw: &[RawEntry]) -> Vec<TreeEntry> {
    let len = raw.len();
    let mut entries = Vec::with_capacity(len);

    for (i, (depth, name, path, is_dir, is_symlink, symlink_target, error)) in
        raw.iter().enumerate()
    {
        let is_last = is_last_sibling(raw, i);

        entries.push(TreeEntry {
            name: name.clone(),
            path: path.clone(),
            depth: *depth,
            is_dir: *is_dir,
            is_symlink: *is_symlink,
            symlink_target: symlink_target.clone(),
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
fn is_last_sibling(raw: &[RawEntry], i: usize) -> bool {
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
