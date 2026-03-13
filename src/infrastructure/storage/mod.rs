//! Storage infrastructure.
//!
//! Provides persistent storage for snapshots, bookmarks, and other data.

pub mod bookmarks;
pub mod retention;
pub mod snapshot_store;

pub use bookmarks::BookmarkStore;
pub use snapshot_store::{SnapshotStore, SnapshotStoreError};
