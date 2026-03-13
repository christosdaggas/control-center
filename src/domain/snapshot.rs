//! Snapshot domain models for known-good system state capture.
//!
//! A snapshot captures the complete system state at a point in time,
//! allowing comparison to identify what changed since a known-good state.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Current schema version for snapshots.
/// Increment when making breaking changes to the snapshot format.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// A complete system state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Unique identifier for this snapshot.
    pub id: Uuid,
    /// User-provided name for this snapshot.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When the snapshot was created.
    pub created_at: DateTime<Utc>,
    /// Schema version for migration support.
    pub schema_version: u32,
    /// Whether sensitive data has been redacted.
    pub redacted: bool,
    /// System identity information.
    pub system_identity: SystemIdentity,
    /// Package state (installed packages and versions).
    pub packages: PackageState,
    /// Systemd unit state.
    pub systemd: SystemdState,
    /// Autostart and timer configuration.
    pub autostart: AutostartState,
    /// Configuration file fingerprints.
    pub config_fingerprints: ConfigFingerprints,
    /// Network baseline.
    pub network: NetworkBaseline,
    /// Storage baseline.
    pub storage: StorageBaseline,
    /// Flatpak applications and runtimes.
    #[serde(default)]
    pub flatpaks: FlatpakState,
    /// Security posture baseline.
    #[serde(default)]
    pub security: SecurityState,
}

impl Snapshot {
    /// Creates a new snapshot with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: None,
            created_at: Utc::now(),
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            redacted: false,
            system_identity: SystemIdentity::default(),
            packages: PackageState::default(),
            systemd: SystemdState::default(),
            autostart: AutostartState::default(),
            config_fingerprints: ConfigFingerprints::default(),
            network: NetworkBaseline::default(),
            storage: StorageBaseline::default(),
            flatpaks: FlatpakState::default(),
            security: SecurityState::default(),
        }
    }

    /// Sets the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Marks the snapshot as redacted.
    #[must_use]
    pub fn with_redaction(mut self, redacted: bool) -> Self {
        self.redacted = redacted;
        self
    }
}

/// System identity information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemIdentity {
    /// Hostname (or redacted placeholder).
    pub hostname: String,
    /// OS name (e.g., "Fedora Linux").
    pub os_name: String,
    /// OS version (e.g., "40").
    pub os_version: String,
    /// Kernel version.
    pub kernel_version: String,
    /// Architecture (e.g., "x86_64").
    pub architecture: String,
}

/// Package state - installed packages and their versions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageState {
    /// Package manager type (rpm, dpkg, etc.).
    pub package_manager: PackageManager,
    /// Map of package name to package info.
    pub packages: BTreeMap<String, PackageInfo>,
}

/// Package manager type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    /// RPM-based (Fedora, RHEL, openSUSE).
    Rpm,
    /// Debian/APT-based (Debian, Ubuntu).
    Dpkg,
    /// Arch Linux pacman.
    Pacman,
    /// Unknown or unsupported.
    #[default]
    Unknown,
}

/// Information about a single installed package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package version.
    pub version: String,
    /// Package release (for RPM).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<String>,
    /// Architecture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    /// Repository/source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Install date if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_date: Option<DateTime<Utc>>,
}

/// Systemd unit state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemdState {
    /// System-level units.
    pub system_units: BTreeMap<String, UnitState>,
    /// User-level units (for the current user).
    pub user_units: BTreeMap<String, UnitState>,
}

/// State of a single systemd unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitState {
    /// Unit type (service, timer, socket, etc.).
    pub unit_type: String,
    /// Whether the unit is enabled.
    pub enabled: EnablementState,
    /// Active state at snapshot time.
    pub active_state: ActiveState,
    /// Whether drop-in overrides exist.
    pub has_overrides: bool,
    /// Description if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Enablement state of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnablementState {
    /// Unit is enabled.
    Enabled,
    /// Unit is disabled.
    Disabled,
    /// Unit is statically enabled (no [Install] section).
    Static,
    /// Unit is masked.
    Masked,
    /// Unit is indirectly enabled.
    Indirect,
    /// Unit is generated.
    Generated,
    /// Unit is an alias.
    Alias,
    /// Unit is transient.
    Transient,
    /// Unknown state.
    Unknown,
}

/// Active state of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActiveState {
    /// Unit is running/active.
    Active,
    /// Unit is inactive.
    Inactive,
    /// Unit is failed.
    Failed,
    /// Unit is activating.
    Activating,
    /// Unit is deactivating.
    Deactivating,
    /// Unknown state.
    Unknown,
}

/// Autostart configuration state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutostartState {
    /// XDG autostart desktop entries.
    pub desktop_entries: Vec<AutostartEntry>,
    /// Systemd user timers.
    pub user_timers: Vec<TimerInfo>,
}

/// A desktop autostart entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutostartEntry {
    /// Filename of the .desktop file.
    pub filename: String,
    /// Application name.
    pub name: String,
    /// Executable command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec: Option<String>,
    /// Whether the entry is hidden (disabled).
    pub hidden: bool,
    /// OnlyShowIn desktop environments.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub only_show_in: Vec<String>,
    /// NotShowIn desktop environments.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub not_show_in: Vec<String>,
}

/// Information about a systemd timer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerInfo {
    /// Timer unit name.
    pub name: String,
    /// Next scheduled run time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<DateTime<Utc>>,
    /// Last run time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
    /// Whether the timer is enabled.
    pub enabled: bool,
}

/// Configuration file fingerprints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConfigFingerprints {
    /// User-specified allowlist of config paths to track.
    pub allowlist: Vec<PathBuf>,
    /// User-specified denylist of config paths to ignore.
    pub denylist: Vec<PathBuf>,
    /// Fingerprints of tracked files.
    pub fingerprints: BTreeMap<PathBuf, ConfigFingerprint>,
}

/// Fingerprint of a single configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFingerprint {
    /// SHA256 hash of file contents.
    pub hash: String,
    /// File modification time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mtime: Option<DateTime<Utc>>,
    /// File size in bytes.
    pub size: u64,
    /// Whether the file exists.
    pub exists: bool,
}

/// Network baseline configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkBaseline {
    /// Default gateway.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_gateway: Option<String>,
    /// Primary DNS servers.
    pub dns_servers: Vec<String>,
    /// Search domains.
    pub search_domains: Vec<String>,
    /// Network interfaces summary.
    pub interfaces: Vec<NetworkInterface>,
}

/// Summary of a network interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name.
    pub name: String,
    /// Whether the interface is up.
    pub is_up: bool,
    /// MAC address (redacted if in redaction mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
    /// IP addresses (redacted if in redaction mode).
    pub ip_addresses: Vec<String>,
}

/// Storage baseline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageBaseline {
    /// Mounted filesystems.
    pub mounts: Vec<MountInfo>,
}

/// Information about a mounted filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountInfo {
    /// Mount point path.
    pub mount_point: String,
    /// Device or source.
    pub device: String,
    /// Filesystem type.
    pub fs_type: String,
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Used bytes.
    pub used_bytes: u64,
    /// Available bytes.
    pub available_bytes: u64,
    /// Usage percentage.
    pub usage_percent: f64,
}

/// Security posture baseline.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityState {
    /// Firewall status.
    pub firewall: FirewallState,
    /// Mandatory access control status.
    pub mac_policy: MacPolicyState,
    /// Secure Boot state.
    pub secure_boot: SecureBootState,
    /// Listening sockets.
    pub listening_sockets: Vec<ListeningSocket>,
    /// SSH exposure summary.
    pub ssh: SshState,
    /// Admin-capable accounts discovered from local groups.
    pub admin_accounts: Vec<AdminAccount>,
    /// Broad Flatpak grants by app id.
    pub flatpak_permissions: BTreeMap<String, FlatpakPermissions>,
}

/// Firewall status.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FirewallState {
    /// Active firewall backend.
    pub backend: FirewallBackend,
    /// Whether a firewall policy appears to be active.
    pub active: bool,
    /// Short status summary, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Supported firewall backends.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallBackend {
    /// `firewalld`.
    Firewalld,
    /// `ufw`.
    Ufw,
    /// `nftables`.
    Nftables,
    /// `iptables`.
    Iptables,
    /// No active backend detected.
    None,
    /// Unknown state.
    #[default]
    Unknown,
}

impl FirewallBackend {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Firewalld => "firewalld",
            Self::Ufw => "ufw",
            Self::Nftables => "nftables",
            Self::Iptables => "iptables",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

/// Mandatory access control state.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MacPolicyState {
    /// SELinux mode.
    pub selinux: PolicyMode,
    /// AppArmor mode.
    pub apparmor: PolicyMode,
    /// Number of AppArmor profiles in enforce mode.
    pub apparmor_enforce_profiles: u32,
    /// Number of AppArmor profiles in complain mode.
    pub apparmor_complain_profiles: u32,
}

/// Security policy enforcement mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    /// Enforcing mode.
    Enforcing,
    /// Permissive mode.
    Permissive,
    /// Complain mode.
    Complain,
    /// Disabled.
    Disabled,
    /// Subsystem not installed.
    NotInstalled,
    /// Unknown state.
    #[default]
    Unknown,
}

impl PolicyMode {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Enforcing => "Enforcing",
            Self::Permissive => "Permissive",
            Self::Complain => "Complain",
            Self::Disabled => "Disabled",
            Self::NotInstalled => "Not installed",
            Self::Unknown => "Unknown",
        }
    }
}

/// Secure Boot status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureBootState {
    /// Secure Boot is enabled.
    Enabled,
    /// Secure Boot is disabled.
    Disabled,
    /// Platform does not expose UEFI Secure Boot state.
    Unsupported,
    /// Unknown state.
    #[default]
    Unknown,
}

impl SecureBootState {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Enabled => "Enabled",
            Self::Disabled => "Disabled",
            Self::Unsupported => "Not available",
            Self::Unknown => "Unknown",
        }
    }
}

/// Summary of a listening socket.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ListeningSocket {
    /// Network protocol, typically `tcp` or `udp`.
    pub protocol: String,
    /// Bound address.
    pub bind_address: String,
    /// Listening port.
    pub port: u16,
    /// Process name, when visible.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    /// Whether the socket is reachable beyond loopback.
    pub public: bool,
}

/// SSH exposure summary.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SshState {
    /// Whether the SSH daemon is active.
    pub service_active: bool,
    /// Whether the SSH daemon is enabled at boot.
    pub service_enabled: bool,
    /// Password authentication status when parseable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_authentication: Option<bool>,
    /// Raw `PermitRootLogin` value when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permit_root_login: Option<String>,
    /// Listening ports attributed to SSH.
    pub listening_ports: Vec<u16>,
    /// Bound addresses attributed to SSH.
    pub listening_addresses: Vec<String>,
}

/// Admin-capable account summary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AdminAccount {
    /// Username.
    pub username: String,
    /// Groups that grant elevated access.
    pub groups: Vec<String>,
}

/// Broad Flatpak grants for one application.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FlatpakPermissions {
    /// Broad permissions that weaken sandboxing.
    pub broad_permissions: Vec<String>,
}

/// Flatpak application state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FlatpakState {
    /// Installed Flatpak applications.
    pub apps: BTreeMap<String, FlatpakApp>,
    /// Installed Flatpak runtimes.
    pub runtimes: BTreeMap<String, FlatpakRuntime>,
    /// Configured remotes.
    pub remotes: Vec<FlatpakRemote>,
}

/// Information about an installed Flatpak application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakApp {
    /// Application name (human-readable).
    pub name: String,
    /// Version string.
    pub version: String,
    /// Branch (e.g., "stable").
    pub branch: String,
    /// Architecture (e.g., "x86_64").
    pub arch: String,
    /// Origin remote (e.g., "flathub").
    pub origin: String,
    /// Installation type ("system" or "user").
    pub installation: FlatpakInstallation,
}

/// Information about an installed Flatpak runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakRuntime {
    /// Runtime name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Branch (e.g., "23.08").
    pub branch: String,
    /// Architecture.
    pub arch: String,
    /// Origin remote.
    pub origin: String,
}

/// Information about a Flatpak remote.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatpakRemote {
    /// Remote name (e.g., "flathub").
    pub name: String,
    /// Remote URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether this is a system or user remote.
    pub installation: FlatpakInstallation,
}

/// Flatpak installation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlatpakInstallation {
    /// System-wide installation.
    #[default]
    System,
    /// User installation.
    User,
}

/// Metadata for a snapshot (used in list views).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Unique identifier.
    pub id: Uuid,
    /// User-provided name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// When created.
    pub created_at: DateTime<Utc>,
    /// Whether redacted.
    pub redacted: bool,
    /// Number of packages.
    pub package_count: usize,
    /// Number of Flatpak apps.
    #[serde(default)]
    pub flatpak_count: usize,
    /// Number of system units.
    pub system_unit_count: usize,
    /// Number of user units.
    pub user_unit_count: usize,
}

impl From<&Snapshot> for SnapshotMetadata {
    fn from(snapshot: &Snapshot) -> Self {
        Self {
            id: snapshot.id,
            name: snapshot.name.clone(),
            description: snapshot.description.clone(),
            created_at: snapshot.created_at,
            redacted: snapshot.redacted,
            package_count: snapshot.packages.packages.len(),
            flatpak_count: snapshot.flatpaks.apps.len(),
            system_unit_count: snapshot.systemd.system_units.len(),
            user_unit_count: snapshot.systemd.user_units.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_creation() {
        let snapshot = Snapshot::new("Test Snapshot")
            .with_description("A test snapshot")
            .with_redaction(false);

        assert_eq!(snapshot.name, "Test Snapshot");
        assert_eq!(snapshot.description, Some("A test snapshot".to_string()));
        assert!(!snapshot.redacted);
        assert_eq!(snapshot.schema_version, SNAPSHOT_SCHEMA_VERSION);
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = Snapshot::new("Serialization Test");
        let json = serde_json::to_string(&snapshot).expect("serialize");
        let parsed: Snapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.name, snapshot.name);
        assert_eq!(parsed.id, snapshot.id);
    }
}
