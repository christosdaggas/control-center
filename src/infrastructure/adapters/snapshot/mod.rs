//! Snapshot data collectors.
//!
//! This module provides trait-based collectors for gathering system state
//! data to create snapshots. Each collector is responsible for one category.

mod autostart;
mod config;
mod flatpak;
mod network;
mod packages;
mod security;
mod storage;
mod systemd_snapshot;
mod system_identity;

pub use autostart::AutostartCollector;
pub use config::ConfigFingerprintCollector;
pub use flatpak::FlatpakCollector;
pub use network::NetworkCollector;
pub use packages::PackageCollector;
pub use security::SecurityCollector;
pub use storage::StorageCollector;
pub use systemd_snapshot::SystemdSnapshotCollector;
pub use system_identity::SystemIdentityCollector;

use crate::domain::snapshot::Snapshot;
use thiserror::Error;

/// Errors that can occur during snapshot collection.
#[derive(Debug, Error)]
pub enum CollectorError {
    /// Command execution failed.
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// Failed to parse output.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// The collector is not available on this system.
    #[error("Collector not available: {0}")]
    NotAvailable(String),
}

/// Trait for snapshot collectors.
///
/// Each collector gathers data for one category of the snapshot.
pub trait SnapshotCollector: Send + Sync {
    /// Returns the name of this collector.
    fn name(&self) -> &'static str;

    /// Returns true if this collector is available on the current system.
    fn is_available(&self) -> bool;

    /// Collects data and updates the snapshot in place.
    fn collect(&self, snapshot: &mut Snapshot, redact: bool) -> Result<(), CollectorError>;
}

/// Registry of all snapshot collectors.
pub struct CollectorRegistry {
    collectors: Vec<Box<dyn SnapshotCollector>>,
}

impl CollectorRegistry {
    /// Creates a new registry with all available collectors.
    #[must_use]
    pub fn new() -> Self {
        let collectors: Vec<Box<dyn SnapshotCollector>> = vec![
            Box::new(SystemIdentityCollector),
            Box::new(PackageCollector::new()),
            Box::new(FlatpakCollector),
            Box::new(SystemdSnapshotCollector),
            Box::new(AutostartCollector),
            Box::new(ConfigFingerprintCollector::with_defaults()),
            Box::new(NetworkCollector),
            Box::new(StorageCollector),
            Box::new(SecurityCollector),
        ];

        Self { collectors }
    }

    /// Collects all data into a snapshot.
    pub fn collect_all(&self, snapshot: &mut Snapshot, redact: bool) -> Vec<CollectorError> {
        let mut errors = Vec::new();

        for collector in &self.collectors {
            if collector.is_available() {
                if let Err(e) = collector.collect(snapshot, redact) {
                    tracing::warn!(
                        collector = collector.name(),
                        error = %e,
                        "Collector failed"
                    );
                    errors.push(e);
                }
            } else {
                tracing::debug!(
                    collector = collector.name(),
                    "Collector not available, skipping"
                );
            }
        }

        errors
    }
}

impl Default for CollectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
