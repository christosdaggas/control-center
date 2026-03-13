//! Core event types and data structures.
//!
//! This module defines the normalized event model that all sources
//! are converted into for unified timeline display.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Classification of system events.
///
/// Each variant represents a distinct category of system activity
/// that users care about when diagnosing issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Package management events
    /// A new package was installed.
    PackageInstall,
    /// An existing package was updated.
    PackageUpdate,
    /// A package was removed.
    PackageRemove,

    // Systemd service events
    /// A service was started.
    ServiceStart,
    /// A service was stopped.
    ServiceStop,
    /// A service was restarted.
    ServiceRestart,
    /// A service failed to start or crashed.
    ServiceFailed,

    // Application events
    /// An application crashed (coredump, SIGSEGV, etc.).
    AppCrash,

    // Kernel events
    /// Kernel warning message.
    KernelWarning,
    /// Kernel error message.
    KernelError,

    // Security/permission events
    /// Permission denied (SELinux, AppArmor, sudo, polkit).
    PermissionDenied,

    // Network events
    /// Network link went down.
    NetworkLinkDown,
    /// Network link came up.
    NetworkLinkUp,
    /// DHCP failed to obtain lease.
    NetworkDhcpFailure,
    /// DNS resolution failed.
    NetworkDnsFailure,

    // Disk events
    /// Disk space warning threshold crossed.
    DiskSpaceWarning,
    /// Disk space critical threshold crossed.
    DiskSpaceCritical,
    /// Inode exhaustion detected.
    DiskInodeExhaustion,

    // System lifecycle events
    /// System boot completed.
    SystemBoot,
    /// System shutdown initiated.
    SystemShutdown,

    // Generic/uncategorized
    /// An event that doesn't fit other categories.
    Other,
}

impl EventType {
    /// Returns a human-readable label for this event type.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::PackageInstall => "Package Installed",
            Self::PackageUpdate => "Package Updated",
            Self::PackageRemove => "Package Removed",
            Self::ServiceStart => "Service Started",
            Self::ServiceStop => "Service Stopped",
            Self::ServiceRestart => "Service Restarted",
            Self::ServiceFailed => "Service Failed",
            Self::AppCrash => "Application Crash",
            Self::KernelWarning => "Kernel Warning",
            Self::KernelError => "Kernel Error",
            Self::PermissionDenied => "Permission Denied",
            Self::NetworkLinkDown => "Network Down",
            Self::NetworkLinkUp => "Network Up",
            Self::NetworkDhcpFailure => "DHCP Failure",
            Self::NetworkDnsFailure => "DNS Failure",
            Self::DiskSpaceWarning => "Disk Space Warning",
            Self::DiskSpaceCritical => "Disk Space Critical",
            Self::DiskInodeExhaustion => "Inode Exhaustion",
            Self::SystemBoot => "System Boot",
            Self::SystemShutdown => "System Shutdown",
            Self::Other => "Other",
        }
    }

    /// Returns the Freedesktop icon name for this event type.
    #[must_use]
    pub const fn icon_name(&self) -> &'static str {
        match self {
            Self::PackageInstall | Self::PackageUpdate | Self::PackageRemove => {
                "package-x-generic-symbolic"
            }
            Self::ServiceStart | Self::ServiceStop | Self::ServiceRestart => {
                "system-run-symbolic"
            }
            Self::ServiceFailed => "dialog-error-symbolic",
            Self::AppCrash => "dialog-error-symbolic",
            Self::KernelWarning => "dialog-warning-symbolic",
            Self::KernelError => "dialog-error-symbolic",
            Self::PermissionDenied => "dialog-password-symbolic",
            Self::NetworkLinkDown | Self::NetworkDhcpFailure | Self::NetworkDnsFailure => {
                "network-offline-symbolic"
            }
            Self::NetworkLinkUp => "network-wired-symbolic",
            Self::DiskSpaceWarning | Self::DiskSpaceCritical | Self::DiskInodeExhaustion => {
                "drive-harddisk-symbolic"
            }
            Self::SystemBoot => "system-reboot-symbolic",
            Self::SystemShutdown => "system-shutdown-symbolic",
            Self::Other => "dialog-information-symbolic",
        }
    }
}

/// Severity levels for events.
///
/// Used for filtering, visual styling, and prioritization.
/// Ordered from least to most severe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational message, no action needed.
    Info,
    /// Warning that may indicate a problem.
    Warning,
    /// Error that caused something to fail.
    Error,
    /// Critical failure requiring immediate attention.
    Critical,
}

impl Severity {
    /// Returns a human-readable label for this severity.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
        }
    }

    /// Returns the CSS class name for styling this severity.
    #[must_use]
    pub const fn css_class(&self) -> &'static str {
        match self {
            Self::Info => "severity-info",
            Self::Warning => "severity-warning",
            Self::Error => "severity-error",
            Self::Critical => "severity-critical",
        }
    }
}

/// The source of evidence for an event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    /// Event came from systemd journal.
    Journald,
    /// Event came from a package manager.
    PackageManager(PackageManagerType),
    /// Event came from kernel ring buffer (dmesg).
    Kernel,
    /// Event came from traditional syslog.
    Syslog,
    /// Event came from filesystem monitoring.
    Filesystem,
}

/// Supported package managers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageManagerType {
    /// DNF (Fedora, RHEL, CentOS).
    Dnf,
    /// APT (Debian, Ubuntu).
    Apt,
    /// Flatpak.
    Flatpak,
    /// RPM directly.
    Rpm,
}

/// Reference to underlying raw data for verification.
///
/// Users can expand events to see this evidence, allowing them to
/// verify the normalized summary against the original source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    /// The source of this evidence.
    pub source: EvidenceSource,

    /// Journal cursor for precise lookup (journald only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,

    /// File path where this evidence was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<PathBuf>,

    /// Original log line(s) or raw content.
    pub raw_content: String,

    /// Line number in the source file, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u64>,
}

impl Evidence {
    /// Creates a new evidence record from journald.
    #[must_use]
    pub fn from_journald(cursor: String, raw_content: String) -> Self {
        Self {
            source: EvidenceSource::Journald,
            cursor: Some(cursor),
            file_path: None,
            raw_content,
            line_number: None,
        }
    }

    /// Creates a new evidence record from a log file.
    #[must_use]
    pub fn from_file(file_path: PathBuf, raw_content: String, line_number: Option<u64>) -> Self {
        Self {
            source: EvidenceSource::Syslog,
            cursor: None,
            file_path: Some(file_path),
            raw_content,
            line_number,
        }
    }
}

/// A normalized system event.
///
/// This is the core data structure that represents any system activity
/// in a consistent format, regardless of the original source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Unique identifier for this event.
    pub id: Uuid,

    /// When this event occurred.
    pub timestamp: DateTime<Utc>,

    /// Classification of this event.
    pub event_type: EventType,

    /// Severity level.
    pub severity: Severity,

    /// Human-readable one-line summary.
    pub summary: String,

    /// Extended explanation or context (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    /// Related systemd unit name, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,

    /// Related package name, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,

    /// Raw source references for verification.
    pub evidence: Vec<Evidence>,
}

impl Event {
    /// Creates a new event with the given required fields.
    #[must_use]
    pub fn new(
        timestamp: DateTime<Utc>,
        event_type: EventType,
        severity: Severity,
        summary: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp,
            event_type,
            severity,
            summary,
            details: None,
            service: None,
            package: None,
            evidence: Vec::new(),
        }
    }

    /// Builder method to add details.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Builder method to add service name.
    #[must_use]
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    /// Builder method to add package name.
    #[must_use]
    pub fn with_package(mut self, package: impl Into<String>) -> Self {
        self.package = Some(package.into());
        self
    }

    /// Builder method to add evidence.
    #[must_use]
    pub fn with_evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Returns true if this event involves the given service.
    #[must_use]
    pub fn involves_service(&self, service_name: &str) -> bool {
        self.service.as_deref() == Some(service_name)
    }

    /// Returns true if this event involves the given package.
    #[must_use]
    pub fn involves_package(&self, package_name: &str) -> bool {
        self.package.as_deref() == Some(package_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            Utc::now(),
            EventType::ServiceFailed,
            Severity::Error,
            "nginx.service failed to start".to_string(),
        )
        .with_service("nginx.service");

        assert_eq!(event.event_type, EventType::ServiceFailed);
        assert_eq!(event.severity, Severity::Error);
        assert!(event.involves_service("nginx.service"));
        assert!(!event.involves_service("apache.service"));
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn test_event_type_labels() {
        assert_eq!(EventType::PackageUpdate.label(), "Package Updated");
        assert_eq!(EventType::ServiceFailed.label(), "Service Failed");
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::new(
            Utc::now(),
            EventType::PackageInstall,
            Severity::Info,
            "Installed vim-9.0".to_string(),
        )
        .with_package("vim");

        let json = serde_json::to_string(&event).expect("serialization should succeed");
        let parsed: Event = serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(parsed.event_type, event.event_type);
        assert_eq!(parsed.package, event.package);
    }
}
