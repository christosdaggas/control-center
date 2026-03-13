//! Domain layer - Core business logic.
//!
//! This module contains pure Rust types and logic for:
//! - Event taxonomy and normalization
//! - Correlation rules and engine
//! - Narrative generation
//! - Noise filtering
//! - System snapshots and diff comparison
//! - Resource pressure and bottleneck diagnosis
//!
//! **Important**: This module must not depend on UI or platform-specific crates.

pub mod correlation;
pub mod diff;
pub mod event;
pub mod filter;
pub mod narrative;
pub mod pressure;
pub mod snapshot;
pub mod taxonomy;

pub use diff::{ChangeType, DiffCategory, DiffEntry, DiffEvidence, Impact, SnapshotDiff};
pub use event::{Event, EventType, Evidence, EvidenceSource, Severity};
pub use filter::{FilterConfig, FilterPreset};
pub use snapshot::{
    ActiveState, AdminAccount, AutostartEntry, AutostartState, ConfigFingerprint,
    ConfigFingerprints, EnablementState, FirewallBackend, FirewallState, FlatpakPermissions,
    ListeningSocket, MacPolicyState, MountInfo, NetworkBaseline, NetworkInterface, PackageInfo,
    PackageManager, PackageState, PolicyMode, SecureBootState, SecurityState, Snapshot,
    SnapshotMetadata, SshState, StorageBaseline, SystemIdentity, SystemdState, TimerInfo,
    UnitState, SNAPSHOT_SCHEMA_VERSION,
};
