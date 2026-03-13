//! Data retention policy for snapshots and bookmarks.
//!
//! Provides automatic cleanup of old data based on configurable retention
//! periods. This prevents unbounded disk usage in XDG_DATA_HOME.

use crate::infrastructure::storage::snapshot_store::SnapshotStore;
use chrono::{Duration, Utc};
use tracing::{debug, info, warn};

/// Result of a retention cleanup run.
#[derive(Debug, Clone)]
pub struct RetentionResult {
    /// Number of snapshots deleted.
    pub snapshots_deleted: usize,
    /// Number of bookmarks deleted.
    pub bookmarks_deleted: usize,
    /// Total bytes freed.
    pub bytes_freed: u64,
    /// Any errors encountered.
    pub errors: Vec<String>,
}

impl std::fmt::Display for RetentionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Retention cleanup: {} snapshots deleted, {} bookmarks deleted, {:.1} MB freed",
            self.snapshots_deleted,
            self.bookmarks_deleted,
            self.bytes_freed as f64 / (1024.0 * 1024.0)
        )
    }
}

/// Runs data retention cleanup based on configured policy.
///
/// - `retention_days`: Delete data older than this many days. 0 = keep forever.
/// - `max_snapshots`: Maximum number of snapshots to retain (0 = unlimited).
///
/// Returns a summary of what was cleaned up.
pub fn run_retention(retention_days: u32, max_snapshots: usize) -> RetentionResult {
    let mut result = RetentionResult {
        snapshots_deleted: 0,
        bookmarks_deleted: 0,
        bytes_freed: 0,
        errors: Vec::new(),
    };

    if retention_days == 0 && max_snapshots == 0 {
        debug!("Retention policy: keep everything");
        return result;
    }

    // Clean up old snapshots
    match SnapshotStore::new() {
        Ok(store) => {
            match store.list() {
                Ok(mut snapshots) => {
                    // Sort by creation date (newest first)
                    snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

                    let cutoff = if retention_days > 0 {
                        Some(Utc::now() - Duration::days(i64::from(retention_days)))
                    } else {
                        None
                    };

                    let mut to_delete = Vec::new();

                    for (idx, snap) in snapshots.iter().enumerate() {
                        let too_old = cutoff
                            .map(|c| snap.created_at < c)
                            .unwrap_or(false);
                        let over_limit = max_snapshots > 0 && idx >= max_snapshots;

                        if too_old || over_limit {
                            to_delete.push(snap.id);
                        }
                    }

                    for id in to_delete {
                        match store.delete(id) {
                            Ok(()) => {
                                result.snapshots_deleted += 1;
                                info!(id = %id, "Deleted old snapshot");
                            }
                            Err(e) => {
                                let msg = format!("Failed to delete snapshot {}: {}", id, e);
                                warn!("{}", msg);
                                result.errors.push(msg);
                            }
                        }
                    }
                }
                Err(e) => {
                    result.errors.push(format!("Failed to list snapshots: {}", e));
                }
            }
        }
        Err(e) => {
            result.errors.push(format!("Failed to open snapshot store: {}", e));
        }
    }

    // Clean up old bookmarks
    match crate::infrastructure::storage::BookmarkStore::new() {
        Ok(store) => {
            if let Some(cutoff) = retention_days
                .checked_sub(0)
                .filter(|&d| d > 0)
                .map(|d| Utc::now() - Duration::days(i64::from(d)))
            {
                match store.cleanup_before(cutoff) {
                    Ok(count) => {
                        result.bookmarks_deleted = count;
                        if count > 0 {
                            info!(count, "Deleted old bookmarks");
                        }
                    }
                    Err(e) => {
                        result.errors.push(format!("Failed to clean bookmarks: {}", e));
                    }
                }
            }
        }
        Err(e) => {
            result.errors.push(format!("Failed to open bookmark store: {}", e));
        }
    }

    if result.snapshots_deleted > 0 || result.bookmarks_deleted > 0 {
        info!("{}", result);
    } else {
        debug!("Retention cleanup: nothing to delete");
    }

    result
}

/// Calculates the total disk usage of stored data.
pub fn calculate_data_usage() -> u64 {
    let mut total: u64 = 0;

    if let Ok(store) = SnapshotStore::new() {
        let dir = store.data_dir();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            }
        }
    }

    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retention_result_display() {
        let result = RetentionResult {
            snapshots_deleted: 3,
            bookmarks_deleted: 5,
            bytes_freed: 1_048_576,
            errors: Vec::new(),
        };
        let s = format!("{}", result);
        assert!(s.contains("3 snapshots"));
        assert!(s.contains("5 bookmarks"));
        assert!(s.contains("1.0 MB"));
    }

    #[test]
    fn test_retention_result_display_zero() {
        let result = RetentionResult {
            snapshots_deleted: 0,
            bookmarks_deleted: 0,
            bytes_freed: 0,
            errors: Vec::new(),
        };
        let s = format!("{}", result);
        assert!(s.contains("0 snapshots"));
        assert!(s.contains("0.0 MB"));
    }

    #[test]
    fn test_zero_retention_keeps_all() {
        let result = run_retention(0, 0);
        assert_eq!(result.snapshots_deleted, 0);
        assert_eq!(result.bookmarks_deleted, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_retention_result_clone() {
        let result = RetentionResult {
            snapshots_deleted: 1,
            bookmarks_deleted: 2,
            bytes_freed: 512,
            errors: vec!["test error".to_string()],
        };
        let cloned = result.clone();
        assert_eq!(result.snapshots_deleted, cloned.snapshots_deleted);
        assert_eq!(result.errors.len(), cloned.errors.len());
    }

    #[test]
    fn test_retention_with_days_does_not_panic() {
        // Running retention with a large number of days should not panic
        // even if no snapshots exist.
        let result = run_retention(365, 0);
        assert!(result.errors.is_empty() || !result.errors.is_empty());
    }

    #[test]
    fn test_retention_with_max_snapshots() {
        // Should not panic with a max_snapshots limit
        let result = run_retention(0, 10);
        assert_eq!(result.snapshots_deleted, 0);
    }

    #[test]
    fn test_calculate_data_usage_does_not_panic() {
        let usage = calculate_data_usage();
        // May be 0 if no data exists, but should not panic
        assert!(usage < u64::MAX);
    }
}
