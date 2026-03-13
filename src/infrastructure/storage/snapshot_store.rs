//! Snapshot storage.
//!
//! Persists snapshots to XDG_DATA_HOME with schema versioning.

use crate::domain::snapshot::{Snapshot, SnapshotMetadata, SNAPSHOT_SCHEMA_VERSION};
use directories::ProjectDirs;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Errors that can occur during snapshot storage operations.
#[derive(Debug, Error)]
pub enum SnapshotStoreError {
    /// Failed to determine storage directory.
    #[error("Could not determine data directory")]
    NoDataDir,

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Snapshot not found.
    #[error("Snapshot not found: {0}")]
    NotFound(Uuid),

    /// Schema version mismatch.
    #[error("Schema version mismatch: expected {expected}, found {found}")]
    SchemaMismatch {
        /// Expected schema version.
        expected: u32,
        /// Found schema version.
        found: u32,
    },
}

/// Storage for snapshots.
pub struct SnapshotStore {
    data_dir: PathBuf,
}

impl SnapshotStore {
    /// Creates a new snapshot store.
    pub fn new() -> Result<Self, SnapshotStoreError> {
        let dirs = ProjectDirs::from("com", "chrisdaggas", "ControlCenter")
            .ok_or(SnapshotStoreError::NoDataDir)?;

        let data_dir = dirs.data_dir().join("snapshots");

        // Create directory if it doesn't exist
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
            debug!(path = ?data_dir, "Created snapshots directory");
        }

        Ok(Self { data_dir })
    }

    /// Creates a store with a custom directory (for testing).
    #[cfg(test)]
    pub fn with_dir(data_dir: PathBuf) -> Result<Self, SnapshotStoreError> {
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
        }
        Ok(Self { data_dir })
    }

    /// Returns the path for a snapshot file.
    fn snapshot_path(&self, id: Uuid) -> PathBuf {
        self.data_dir.join(format!("{}.json", id))
    }

    /// Saves a snapshot.
    pub fn save(&self, snapshot: &Snapshot) -> Result<(), SnapshotStoreError> {
        let path = self.snapshot_path(snapshot.id);
        let json = serde_json::to_string_pretty(snapshot)?;
        fs::write(&path, json)?;
        info!(id = %snapshot.id, name = %snapshot.name, "Saved snapshot");
        Ok(())
    }

    /// Loads a snapshot by ID.
    pub fn load(&self, id: Uuid) -> Result<Snapshot, SnapshotStoreError> {
        let path = self.snapshot_path(id);

        if !path.exists() {
            return Err(SnapshotStoreError::NotFound(id));
        }

        let json = fs::read_to_string(&path)?;
        let snapshot: Snapshot = serde_json::from_str(&json)?;

        // Check schema version
        if snapshot.schema_version > SNAPSHOT_SCHEMA_VERSION {
            warn!(
                snapshot_version = snapshot.schema_version,
                current_version = SNAPSHOT_SCHEMA_VERSION,
                "Snapshot has newer schema version"
            );
        }

        debug!(id = %id, name = %snapshot.name, "Loaded snapshot");
        Ok(snapshot)
    }

    /// Deletes a snapshot.
    pub fn delete(&self, id: Uuid) -> Result<(), SnapshotStoreError> {
        let path = self.snapshot_path(id);

        if !path.exists() {
            return Err(SnapshotStoreError::NotFound(id));
        }

        fs::remove_file(&path)?;
        info!(id = %id, "Deleted snapshot");
        Ok(())
    }

    /// Lists all snapshots (metadata only for performance).
    pub fn list(&self) -> Result<Vec<SnapshotMetadata>, SnapshotStoreError> {
        let mut snapshots = Vec::new();

        for entry in fs::read_dir(&self.data_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            match self.load_metadata(&path) {
                Ok(metadata) => snapshots.push(metadata),
                Err(e) => {
                    warn!(path = ?path, error = %e, "Failed to load snapshot metadata");
                }
            }
        }

        // Sort by creation date (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        debug!(count = snapshots.len(), "Listed snapshots");
        Ok(snapshots)
    }

    /// Loads just the metadata from a snapshot file.
    fn load_metadata(&self, path: &PathBuf) -> Result<SnapshotMetadata, SnapshotStoreError> {
        let json = fs::read_to_string(path)?;
        let snapshot: Snapshot = serde_json::from_str(&json)?;
        Ok(SnapshotMetadata::from(&snapshot))
    }

    /// Returns the storage directory path.
    #[must_use]
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Checks if a snapshot exists.
    #[must_use]
    pub fn exists(&self, id: Uuid) -> bool {
        self.snapshot_path(id).exists()
    }
}

impl Default for SnapshotStore {
    fn default() -> Self {
        Self::new().expect("Failed to create snapshot store")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let store = SnapshotStore::with_dir(dir.path().to_path_buf()).unwrap();

        let snapshot = Snapshot::new("Test Snapshot");
        let id = snapshot.id;

        store.save(&snapshot).unwrap();
        assert!(store.exists(id));

        let loaded = store.load(id).unwrap();
        assert_eq!(loaded.name, "Test Snapshot");
        assert_eq!(loaded.id, id);
    }

    #[test]
    fn test_list_snapshots() {
        let dir = tempdir().unwrap();
        let store = SnapshotStore::with_dir(dir.path().to_path_buf()).unwrap();

        store.save(&Snapshot::new("Snapshot 1")).unwrap();
        store.save(&Snapshot::new("Snapshot 2")).unwrap();
        store.save(&Snapshot::new("Snapshot 3")).unwrap();

        let list = store.list().unwrap();
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_delete_snapshot() {
        let dir = tempdir().unwrap();
        let store = SnapshotStore::with_dir(dir.path().to_path_buf()).unwrap();

        let snapshot = Snapshot::new("To Delete");
        let id = snapshot.id;

        store.save(&snapshot).unwrap();
        assert!(store.exists(id));

        store.delete(id).unwrap();
        assert!(!store.exists(id));
    }

    #[test]
    fn test_load_not_found() {
        let dir = tempdir().unwrap();
        let store = SnapshotStore::with_dir(dir.path().to_path_buf()).unwrap();

        let result = store.load(Uuid::new_v4());
        assert!(matches!(result, Err(SnapshotStoreError::NotFound(_))));
    }
}
