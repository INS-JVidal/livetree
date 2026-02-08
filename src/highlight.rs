//! Per-file highlight expiration tracking.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long a per-file highlight stays visible.
pub const HIGHLIGHT_DURATION: Duration = Duration::from_secs(3);

/// Tracks recently changed paths with per-entry expiration.
pub struct HighlightTracker {
    entries: HashMap<PathBuf, Instant>,
}

impl HighlightTracker {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Record a path as highlighted at the given instant.
    pub fn insert(&mut self, path: PathBuf, now: Instant) {
        self.entries.insert(path, now);
    }

    /// Return the set of paths whose highlights have not yet expired.
    pub fn active_set(&mut self, now: Instant) -> HashSet<PathBuf> {
        self.entries
            .retain(|_, inserted| now.duration_since(*inserted) < HIGHLIGHT_DURATION);
        self.entries.keys().cloned().collect()
    }

    /// Remove all highlights (used by the reset key).
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
impl HighlightTracker {
    /// Whether there are any tracked entries (before expiration pruning).
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_active() {
        let mut tracker = HighlightTracker::new();
        let now = Instant::now();
        tracker.insert(PathBuf::from("/tmp/a.txt"), now);
        tracker.insert(PathBuf::from("/tmp/b.txt"), now);

        let active = tracker.active_set(now);
        assert_eq!(active.len(), 2);
        assert!(active.contains(&PathBuf::from("/tmp/a.txt")));
        assert!(active.contains(&PathBuf::from("/tmp/b.txt")));
    }

    #[test]
    fn test_expiry() {
        let mut tracker = HighlightTracker::new();
        let now = Instant::now();
        tracker.insert(PathBuf::from("/tmp/old.txt"), now);

        let later = now + HIGHLIGHT_DURATION + Duration::from_millis(1);
        let active = tracker.active_set(later);
        assert!(active.is_empty(), "Expired entry should be pruned");
        assert!(tracker.is_empty(), "Internal map should be empty after pruning");
    }

    #[test]
    fn test_clear() {
        let mut tracker = HighlightTracker::new();
        let now = Instant::now();
        tracker.insert(PathBuf::from("/tmp/a.txt"), now);
        tracker.insert(PathBuf::from("/tmp/b.txt"), now);

        tracker.clear();
        assert!(tracker.is_empty());
        let active = tracker.active_set(now);
        assert!(active.is_empty());
    }

    #[test]
    fn test_retouch_resets_timer() {
        let mut tracker = HighlightTracker::new();
        let t0 = Instant::now();
        tracker.insert(PathBuf::from("/tmp/a.txt"), t0);

        // Re-insert at a later time (before original would expire)
        let t1 = t0 + Duration::from_secs(2);
        tracker.insert(PathBuf::from("/tmp/a.txt"), t1);

        // At t0 + 3.5s, original would have expired but re-touch keeps it alive
        let t2 = t0 + Duration::from_millis(3500);
        let active = tracker.active_set(t2);
        assert_eq!(active.len(), 1, "Re-touched entry should still be active");
    }

    #[test]
    fn test_mixed_expiry() {
        let mut tracker = HighlightTracker::new();
        let t0 = Instant::now();
        tracker.insert(PathBuf::from("/tmp/old.txt"), t0);

        let t1 = t0 + Duration::from_secs(2);
        tracker.insert(PathBuf::from("/tmp/new.txt"), t1);

        // At t0 + 3.5s: old expired (3.5s > 3s), new still active (1.5s < 3s)
        let t2 = t0 + Duration::from_millis(3500);
        let active = tracker.active_set(t2);
        assert_eq!(active.len(), 1);
        assert!(active.contains(&PathBuf::from("/tmp/new.txt")));
        assert!(!active.contains(&PathBuf::from("/tmp/old.txt")));
    }
}
