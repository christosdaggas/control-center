//! Infrastructure layer - System integrations.
//!
//! This module contains adapters for reading from system sources:
//! - Journald (systemd journal)
//! - Package managers (DNF, APT, Flatpak)
//! - Kernel messages
//! - Desktop environment detection
//! - Icon resolution
//! - Snapshot storage

pub mod adapters;
pub mod desktop;
pub mod icons;
pub mod storage;

pub use desktop::detector::{DesktopEnvironment, SessionType};
pub use storage::{SnapshotStore, SnapshotStoreError};

