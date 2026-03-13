//! Event bookmarks persistence.
//!
//! Stores user-bookmarked event IDs and optional notes in an XDG data directory.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use tracing::debug;
use uuid::Uuid;

/// Errors from bookmark operations.
#[derive(Debug, Error)]
pub enum BookmarkError {
    /// Failed to determine data directory.
    #[error("Could not determine XDG data directory")]
    NoDataDir,

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// A single bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    /// The bookmarked event ID.
    pub event_id: Uuid,
    /// Optional user annotation.
    pub note: Option<String>,
    /// When the bookmark was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Collection of bookmarks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BookmarkStore {
    /// Map from event ID to bookmark.
    pub bookmarks: HashMap<Uuid, Bookmark>,
}

impl BookmarkStore {
    /// Returns the path to the bookmarks file.
    fn bookmarks_path() -> Result<PathBuf, BookmarkError> {
        let dirs = ProjectDirs::from("com", "chrisdaggas", "ControlCenter")
            .ok_or(BookmarkError::NoDataDir)?;
        Ok(dirs.data_dir().join("bookmarks.json"))
    }

    /// Loads bookmarks from disk.
    ///
    /// # Errors
    ///
    /// Returns `BookmarkError` if loading fails.
    pub fn load() -> Result<Self, BookmarkError> {
        let path = Self::bookmarks_path()?;
        if !path.exists() {
            debug!("No bookmarks file, returning empty store");
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path)?;
        let store: Self = serde_json::from_str(&contents)?;
        debug!(count = store.bookmarks.len(), "Loaded bookmarks");
        Ok(store)
    }

    /// Saves bookmarks to disk.
    ///
    /// # Errors
    ///
    /// Returns `BookmarkError` if saving fails.
    pub fn save(&self) -> Result<(), BookmarkError> {
        let path = Self::bookmarks_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        debug!(count = self.bookmarks.len(), "Saved bookmarks");
        Ok(())
    }

    /// Toggles a bookmark for the given event ID.
    ///
    /// Returns `true` if the bookmark was added, `false` if removed.
    pub fn toggle(&mut self, event_id: Uuid) -> bool {
        if self.bookmarks.contains_key(&event_id) {
            self.bookmarks.remove(&event_id);
            false
        } else {
            self.bookmarks.insert(
                event_id,
                Bookmark {
                    event_id,
                    note: None,
                    created_at: chrono::Utc::now(),
                },
            );
            true
        }
    }

    /// Returns true if the given event is bookmarked.
    #[must_use]
    pub fn is_bookmarked(&self, event_id: &Uuid) -> bool {
        self.bookmarks.contains_key(event_id)
    }

    /// Sets or updates the note for a bookmark.
    pub fn set_note(&mut self, event_id: Uuid, note: Option<String>) {
        if let Some(bookmark) = self.bookmarks.get_mut(&event_id) {
            bookmark.note = note;
        }
    }

    /// Returns the count of bookmarks.
    #[must_use]
    pub fn count(&self) -> usize {
        self.bookmarks.len()
    }

    /// Deletes bookmarks created before the given cutoff date.
    ///
    /// Returns the number of bookmarks removed.
    ///
    /// # Errors
    ///
    /// Returns `BookmarkError` if saving fails.
    pub fn cleanup_before(&self, cutoff: chrono::DateTime<chrono::Utc>) -> Result<usize, BookmarkError> {
        let mut store = self.clone();
        let before = store.bookmarks.len();
        store.bookmarks.retain(|_, b| b.created_at >= cutoff);
        let removed = before - store.bookmarks.len();
        if removed > 0 {
            store.save()?;
        }
        Ok(removed)
    }

    /// Creates a new BookmarkStore by loading from disk.
    ///
    /// # Errors
    ///
    /// Returns `BookmarkError` if loading fails.
    pub fn new() -> Result<Self, BookmarkError> {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_bookmark() {
        let mut store = BookmarkStore::default();
        let id = Uuid::new_v4();

        assert!(!store.is_bookmarked(&id));
        assert!(store.toggle(id)); // added
        assert!(store.is_bookmarked(&id));
        assert!(!store.toggle(id)); // removed
        assert!(!store.is_bookmarked(&id));
    }

    #[test]
    fn test_set_note() {
        let mut store = BookmarkStore::default();
        let id = Uuid::new_v4();

        store.toggle(id);
        store.set_note(id, Some("Important event".to_string()));

        assert_eq!(
            store.bookmarks.get(&id).unwrap().note.as_deref(),
            Some("Important event")
        );
    }

    #[test]
    fn test_count() {
        let mut store = BookmarkStore::default();
        assert_eq!(store.count(), 0);

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        store.toggle(id1);
        assert_eq!(store.count(), 1);
        store.toggle(id2);
        assert_eq!(store.count(), 2);
        store.toggle(id1); // remove
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_set_note_on_nonexistent_bookmark() {
        let mut store = BookmarkStore::default();
        let id = Uuid::new_v4();
        // Should not panic when setting note on non-existent bookmark
        store.set_note(id, Some("note".to_string()));
        assert!(!store.is_bookmarked(&id));
    }

    #[test]
    fn test_clear_note() {
        let mut store = BookmarkStore::default();
        let id = Uuid::new_v4();
        store.toggle(id);
        store.set_note(id, Some("note".to_string()));
        store.set_note(id, None);
        assert!(store.bookmarks.get(&id).unwrap().note.is_none());
    }

    #[test]
    fn test_bookmark_has_created_at() {
        let mut store = BookmarkStore::default();
        let id = Uuid::new_v4();
        let before = chrono::Utc::now();
        store.toggle(id);
        let after = chrono::Utc::now();
        let bookmark = store.bookmarks.get(&id).unwrap();
        assert!(bookmark.created_at >= before);
        assert!(bookmark.created_at <= after);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut store = BookmarkStore::default();
        let id = Uuid::new_v4();
        store.toggle(id);
        store.set_note(id, Some("test note".to_string()));

        let json = serde_json::to_string(&store).unwrap();
        let deserialized: BookmarkStore = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.count(), 1);
        assert!(deserialized.is_bookmarked(&id));
        assert_eq!(
            deserialized.bookmarks.get(&id).unwrap().note.as_deref(),
            Some("test note")
        );
    }

    #[test]
    fn test_toggle_multiple_bookmarks() {
        let mut store = BookmarkStore::default();
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        for &id in &ids {
            store.toggle(id);
        }
        assert_eq!(store.count(), 5);

        for &id in &ids {
            assert!(store.is_bookmarked(&id));
        }
    }
}
