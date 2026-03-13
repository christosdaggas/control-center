//! Event source adapters.
//!
//! Each adapter reads from a specific system source and normalizes
//! events into the common Event model.

pub mod custom_log;
pub mod flatpak;
pub mod journald;
pub mod kernel;
pub mod package;
pub mod pressure;
pub mod process;
pub mod security;
pub mod snapshot;
pub mod systemd;
pub mod system_stats;

pub use pressure::{
    BufferStats, PressureRingBuffer, PressureSampler, PsiAdapter, PsiAvailability,
    SampleGranularity, SamplerCapabilities, SharedRingBuffer, SharedSampler, SystemdUnit as PressureUnit,
    UnitMapper, UnitStats,
};
pub use security::{SecurityAdapter, SecurityFinding, SecurityPosture};
pub use snapshot::{CollectorError, CollectorRegistry, SnapshotCollector};
pub use systemd::{EnabledState, SystemdAdapter, SystemdUnit, UnitState, UnitType};
pub use system_stats::{CpuStats, DiskStats, MemoryStats, SystemHealth, SystemStatsAdapter, UptimeInfo, LoadAverage};

use crate::domain::event::Event;
use chrono::{DateTime, Utc};
use thiserror::Error;

/// Errors that can occur when reading from adapters.
#[derive(Debug, Error)]
pub enum AdapterError {
    /// Failed to access the journal.
    #[error("Journal access failed: {0}")]
    JournalError(String),

    /// Failed to read package history.
    #[error("Package history read failed: {0}")]
    PackageHistoryError(String),

    /// Failed to read kernel messages.
    #[error("Kernel message read failed: {0}")]
    KernelError(String),

    /// The requested source is not available.
    #[error("Source not available: {0}")]
    NotAvailable(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Trait for event source adapters.
///
/// All adapters must implement this trait to provide a consistent
/// interface for event ingestion.
pub trait EventAdapter: Send + Sync {
    /// Returns the name of this adapter (e.g., "journald", "dnf").
    fn name(&self) -> &'static str;

    /// Returns true if this adapter is available on the current system.
    fn is_available(&self) -> bool;

    /// Reads events since the given timestamp.
    ///
    /// Returns events in chronological order.
    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError>;

    /// Reads events from the last N hours.
    fn read_last_hours(&self, hours: u32) -> Result<Vec<Event>, AdapterError> {
        let since = Utc::now() - chrono::Duration::hours(i64::from(hours));
        self.read_since(since)
    }
}

/// Registry of available adapters.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn EventAdapter>>,
}

impl AdapterRegistry {
    /// Creates a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Registers an adapter.
    pub fn register(&mut self, adapter: Box<dyn EventAdapter>) {
        self.adapters.push(adapter);
    }

    /// Returns all available adapters.
    #[must_use]
    pub fn available_adapters(&self) -> Vec<&dyn EventAdapter> {
        self.adapters
            .iter()
            .filter(|a| a.is_available())
            .map(|a| a.as_ref())
            .collect()
    }

    /// Reads events from all available adapters.
    pub fn read_all_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        let mut all_events = Vec::new();

        for adapter in self.available_adapters() {
            match adapter.read_since(since) {
                Ok(events) => all_events.extend(events),
                Err(e) => {
                    tracing::warn!(
                        adapter = adapter.name(),
                        error = %e,
                        "Failed to read from adapter"
                    );
                }
            }
        }

        // Sort all events by timestamp
        all_events.sort_by_key(|e| e.timestamp);

        Ok(all_events)
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a registry with all built-in adapters.
#[must_use]
pub fn create_default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();

    registry.register(Box::new(journald::JournaldAdapter::new()));
    registry.register(Box::new(package::dnf::DnfAdapter::new()));
    registry.register(Box::new(package::apt::AptAdapter::new()));
    registry.register(Box::new(package::pacman::PacmanAdapter::new()));
    registry.register(Box::new(package::zypper::ZypperAdapter::new()));
    registry.register(Box::new(kernel::KernelAdapter::new()));
    registry.register(Box::new(flatpak::FlatpakAdapter::new()));

    registry
}
