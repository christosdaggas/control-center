//! Diff domain models for comparing snapshots.
//!
//! This module defines the structures for representing differences
//! between two snapshots in a deterministic, explainable format.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result of comparing two snapshots.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotDiff {
    /// Base snapshot ID (the "before" state).
    pub base_snapshot_id: uuid::Uuid,
    /// Base snapshot name.
    pub base_snapshot_name: String,
    /// Current state description (either snapshot name or "Current System").
    pub current_state_name: String,
    /// Package changes.
    pub packages: Vec<DiffEntry>,
    /// Flatpak changes.
    #[serde(default)]
    pub flatpaks: Vec<DiffEntry>,
    /// Systemd unit changes.
    pub systemd: Vec<DiffEntry>,
    /// Autostart/timer changes.
    pub autostart: Vec<DiffEntry>,
    /// Config file changes.
    pub config: Vec<DiffEntry>,
    /// Network changes.
    pub network: Vec<DiffEntry>,
    /// Storage changes.
    pub storage: Vec<DiffEntry>,
    /// Security posture changes.
    #[serde(default)]
    pub security: Vec<DiffEntry>,
}

impl SnapshotDiff {
    /// Creates a new empty diff.
    #[must_use]
    pub fn new(
        base_id: uuid::Uuid,
        base_name: impl Into<String>,
        current_name: impl Into<String>,
    ) -> Self {
        Self {
            base_snapshot_id: base_id,
            base_snapshot_name: base_name.into(),
            current_state_name: current_name.into(),
            packages: Vec::new(),
            flatpaks: Vec::new(),
            systemd: Vec::new(),
            autostart: Vec::new(),
            config: Vec::new(),
            network: Vec::new(),
            storage: Vec::new(),
            security: Vec::new(),
        }
    }

    /// Returns the total number of changes across all categories.
    #[must_use]
    pub fn total_changes(&self) -> usize {
        self.packages.len()
            + self.flatpaks.len()
            + self.systemd.len()
            + self.autostart.len()
            + self.config.len()
            + self.network.len()
            + self.storage.len()
            + self.security.len()
    }

    /// Returns the number of high-impact changes.
    #[must_use]
    pub fn high_impact_count(&self) -> usize {
        self.all_entries()
            .filter(|e| e.impact == Impact::High)
            .count()
    }

    /// Returns an iterator over all entries across all categories.
    pub fn all_entries(&self) -> impl Iterator<Item = &DiffEntry> {
        self.packages
            .iter()
            .chain(self.flatpaks.iter())
            .chain(self.systemd.iter())
            .chain(self.autostart.iter())
            .chain(self.config.iter())
            .chain(self.network.iter())
            .chain(self.storage.iter())
            .chain(self.security.iter())
    }

    /// Filters entries by category.
    #[must_use]
    pub fn entries_by_category(&self, category: DiffCategory) -> &[DiffEntry] {
        match category {
            DiffCategory::Packages => &self.packages,
            DiffCategory::Flatpaks => &self.flatpaks,
            DiffCategory::Systemd => &self.systemd,
            DiffCategory::Autostart => &self.autostart,
            DiffCategory::Config => &self.config,
            DiffCategory::Network => &self.network,
            DiffCategory::Storage => &self.storage,
            DiffCategory::Security => &self.security,
        }
    }

    /// Filters entries by impact level.
    pub fn entries_by_impact(&self, impact: Impact) -> Vec<&DiffEntry> {
        self.all_entries().filter(|e| e.impact == impact).collect()
    }
}

/// A single difference entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    /// Category of this change.
    pub category: DiffCategory,
    /// Type of change.
    pub change_type: ChangeType,
    /// Human-readable name of the changed item.
    pub name: String,
    /// Value before the change (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Value after the change (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Impact assessment.
    pub impact: Impact,
    /// Human-readable explanation of why this matters.
    pub explanation: String,
    /// Raw evidence (log lines, command output, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<DiffEvidence>,
}

impl DiffEntry {
    /// Creates a new diff entry.
    #[must_use]
    pub fn new(
        category: DiffCategory,
        change_type: ChangeType,
        name: impl Into<String>,
        impact: Impact,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            category,
            change_type,
            name: name.into(),
            before: None,
            after: None,
            impact,
            explanation: explanation.into(),
            evidence: None,
        }
    }

    /// Sets the before value.
    #[must_use]
    pub fn with_before(mut self, before: impl Into<String>) -> Self {
        self.before = Some(before.into());
        self
    }

    /// Sets the after value.
    #[must_use]
    pub fn with_after(mut self, after: impl Into<String>) -> Self {
        self.after = Some(after.into());
        self
    }

    /// Sets the evidence.
    #[must_use]
    pub fn with_evidence(mut self, evidence: DiffEvidence) -> Self {
        self.evidence = Some(evidence);
        self
    }
}

/// Category of a diff entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffCategory {
    /// Package changes (install, update, remove).
    Packages,
    /// Flatpak app changes (install, update, remove).
    Flatpaks,
    /// Systemd unit changes (enabled, disabled, failed).
    Systemd,
    /// Autostart/timer changes.
    Autostart,
    /// Configuration file changes.
    Config,
    /// Network configuration changes.
    Network,
    /// Storage/mount changes.
    Storage,
    /// Security posture changes.
    Security,
}

impl DiffCategory {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Packages => "Packages",
            Self::Flatpaks => "Flatpak Apps",
            Self::Systemd => "Services & Units",
            Self::Autostart => "Startup & Timers",
            Self::Config => "Configuration Files",
            Self::Network => "Network",
            Self::Storage => "Storage",
            Self::Security => "Security Posture",
        }
    }

    /// Returns an icon name for this category.
    #[must_use]
    pub const fn icon_name(&self) -> &'static str {
        match self {
            Self::Packages => "package-x-generic-symbolic",
            Self::Flatpaks => "application-x-executable-symbolic",
            Self::Systemd => "system-run-symbolic",
            Self::Autostart => "alarm-symbolic",
            Self::Config => "document-properties-symbolic",
            Self::Network => "network-wired-symbolic",
            Self::Storage => "drive-harddisk-symbolic",
            Self::Security => "security-high-symbolic",
        }
    }

    /// Returns all categories in display order.
    #[must_use]
    pub const fn all() -> &'static [DiffCategory] {
        &[
            Self::Packages,
            Self::Flatpaks,
            Self::Systemd,
            Self::Autostart,
            Self::Config,
            Self::Network,
            Self::Storage,
            Self::Security,
        ]
    }
}

/// Type of change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    /// Item was added.
    Added,
    /// Item was removed.
    Removed,
    /// Item was modified.
    Modified,
}

impl ChangeType {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Removed => "Removed",
            Self::Modified => "Modified",
        }
    }

    /// Returns a CSS class name for styling.
    #[must_use]
    pub const fn css_class(&self) -> &'static str {
        match self {
            Self::Added => "diff-added",
            Self::Removed => "diff-removed",
            Self::Modified => "diff-modified",
        }
    }
}

/// Impact level of a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    /// Low impact - informational change.
    Low,
    /// Medium impact - may affect system behavior.
    Medium,
    /// High impact - likely to cause issues.
    High,
}

impl Impact {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    /// Returns a CSS class name for styling.
    #[must_use]
    pub const fn css_class(&self) -> &'static str {
        match self {
            Self::Low => "impact-low",
            Self::Medium => "impact-medium",
            Self::High => "impact-high",
        }
    }
}

/// Evidence for a diff entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEvidence {
    /// Source of the evidence.
    pub source: String,
    /// Raw content (command output, file content, etc.).
    pub content: String,
    /// File path if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<PathBuf>,
}

impl DiffEvidence {
    /// Creates new evidence from raw content.
    #[must_use]
    pub fn new(source: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            content: content.into(),
            file_path: None,
        }
    }

    /// Sets the file path.
    #[must_use]
    pub fn with_file_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.file_path = Some(path.into());
        self
    }
}

/// Impact heuristics rules for determining change severity.
///
/// These rules are deterministic and documented for transparency.
pub mod impact_rules {
    use super::*;

    /// Rule: Security-related package changes are high impact.
    pub const SECURITY_PACKAGE_KEYWORDS: &[&str] = &[
        "openssl",
        "gnutls",
        "nss",
        "ca-certificates",
        "selinux",
        "pam",
        "sudo",
        "polkit",
        "firewalld",
        "iptables",
        "nftables",
        "ssh",
        "gpg",
        "gnupg",
    ];

    /// Rule: Critical service names that affect system stability.
    pub const CRITICAL_SERVICES: &[&str] = &[
        "systemd",
        "NetworkManager",
        "dbus",
        "polkit",
        "udev",
        "firewalld",
        "sshd",
        "auditd",
        "journald",
    ];

    /// Rule: Configuration files that are high impact if changed.
    pub const HIGH_IMPACT_CONFIG_PATHS: &[&str] = &[
        "/etc/passwd",
        "/etc/shadow",
        "/etc/group",
        "/etc/sudoers",
        "/etc/ssh/sshd_config",
        "/etc/fstab",
        "/etc/hosts",
        "/etc/resolv.conf",
        "/etc/selinux/config",
    ];

    /// Determines impact for a package change.
    #[must_use]
    pub fn package_impact(name: &str, change_type: ChangeType) -> Impact {
        let name_lower = name.to_lowercase();

        // Security packages are always high impact
        for keyword in SECURITY_PACKAGE_KEYWORDS {
            if name_lower.contains(keyword) {
                return Impact::High;
            }
        }

        // Kernel updates are high impact
        if name_lower.starts_with("kernel") || name_lower.starts_with("linux-image") {
            return Impact::High;
        }

        // Removals are generally more impactful than additions
        match change_type {
            ChangeType::Removed => Impact::Medium,
            ChangeType::Modified => Impact::Low,
            ChangeType::Added => Impact::Low,
        }
    }

    /// Determines impact for a systemd unit change.
    #[must_use]
    pub fn systemd_impact(unit_name: &str, change_type: ChangeType, is_failed: bool) -> Impact {
        // Failed services are always high impact
        if is_failed {
            return Impact::High;
        }

        // Check for critical services
        for critical in CRITICAL_SERVICES {
            if unit_name.contains(critical) {
                return Impact::High;
            }
        }

        // Service becoming disabled is medium impact
        match change_type {
            ChangeType::Removed => Impact::Medium,
            ChangeType::Modified => Impact::Medium,
            ChangeType::Added => Impact::Low,
        }
    }

    /// Determines impact for a config file change.
    #[must_use]
    pub fn config_impact(path: &str) -> Impact {
        for high_impact in HIGH_IMPACT_CONFIG_PATHS {
            if path == *high_impact || path.starts_with(high_impact) {
                return Impact::High;
            }
        }

        // SSH config changes are high impact
        if path.contains("/ssh/") || path.contains(".ssh/") {
            return Impact::High;
        }

        // System config changes are medium impact
        if path.starts_with("/etc/") {
            return Impact::Medium;
        }

        Impact::Low
    }

    /// Determines impact for a network change.
    #[must_use]
    pub fn network_impact(is_gateway: bool, is_dns: bool) -> Impact {
        if is_gateway {
            return Impact::High;
        }
        if is_dns {
            return Impact::Medium;
        }
        Impact::Low
    }

    /// Determines impact for a storage change.
    #[must_use]
    pub fn storage_impact(mount_point: &str, usage_delta_percent: f64) -> Impact {
        // Root filesystem changes are high impact
        if mount_point == "/" {
            if usage_delta_percent > 10.0 {
                return Impact::High;
            }
            return Impact::Medium;
        }

        // Large usage changes are concerning
        if usage_delta_percent > 20.0 {
            return Impact::Medium;
        }

        Impact::Low
    }

    /// Determines impact for a listening socket exposure change.
    #[must_use]
    pub fn security_socket_impact(port: u16, public: bool) -> Impact {
        if !public {
            return Impact::Low;
        }

        if matches!(port, 22 | 2375 | 3306 | 5432 | 6379 | 27017) {
            return Impact::High;
        }

        Impact::Medium
    }

    /// Determines impact for a Flatpak permission change.
    #[must_use]
    pub fn security_permission_impact(permission: &str) -> Impact {
        if permission.starts_with("filesystem=host")
            || permission.starts_with("filesystem=home")
            || permission == "devices=all"
        {
            return Impact::High;
        }

        if permission == "network" || permission.starts_with("system-bus=talk:") {
            return Impact::Medium;
        }

        Impact::Low
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_entry_builder() {
        let entry = DiffEntry::new(
            DiffCategory::Packages,
            ChangeType::Modified,
            "openssl",
            Impact::High,
            "Security library updated",
        )
        .with_before("1.1.1")
        .with_after("3.0.0");

        assert_eq!(entry.name, "openssl");
        assert_eq!(entry.before, Some("1.1.1".to_string()));
        assert_eq!(entry.after, Some("3.0.0".to_string()));
        assert_eq!(entry.impact, Impact::High);
    }

    #[test]
    fn test_impact_ordering() {
        assert!(Impact::Low < Impact::Medium);
        assert!(Impact::Medium < Impact::High);
    }

    #[test]
    fn test_package_impact_rules() {
        assert_eq!(
            impact_rules::package_impact("openssl", ChangeType::Modified),
            Impact::High
        );
        assert_eq!(
            impact_rules::package_impact("vim", ChangeType::Modified),
            Impact::Low
        );
        assert_eq!(
            impact_rules::package_impact("vim", ChangeType::Removed),
            Impact::Medium
        );
    }

    #[test]
    fn test_snapshot_diff_counts() {
        let mut diff = SnapshotDiff::new(uuid::Uuid::new_v4(), "Base", "Current");
        diff.packages.push(DiffEntry::new(
            DiffCategory::Packages,
            ChangeType::Added,
            "test",
            Impact::Low,
            "Test package",
        ));
        diff.systemd.push(DiffEntry::new(
            DiffCategory::Systemd,
            ChangeType::Modified,
            "sshd",
            Impact::High,
            "SSH daemon modified",
        ));
        diff.security.push(DiffEntry::new(
            DiffCategory::Security,
            ChangeType::Modified,
            "Firewall",
            Impact::Medium,
            "Firewall backend changed",
        ));

        assert_eq!(diff.total_changes(), 3);
        assert_eq!(diff.high_impact_count(), 1);
    }
}
