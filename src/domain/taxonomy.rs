//! Event taxonomy and classification helpers.
//!
//! Provides utilities for categorizing and grouping events.

use super::event::EventType;

/// Categories for grouping related event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventCategory {
    /// Package management operations.
    Package,
    /// Systemd service lifecycle.
    Service,
    /// Application behavior.
    Application,
    /// Kernel messages.
    Kernel,
    /// Security and permissions.
    Security,
    /// Network connectivity.
    Network,
    /// Disk and storage.
    Disk,
    /// System lifecycle (boot, shutdown).
    System,
    /// Uncategorized events.
    Other,
}

impl EventCategory {
    /// Returns the category for a given event type.
    #[must_use]
    pub const fn for_event_type(event_type: EventType) -> Self {
        match event_type {
            EventType::PackageInstall
            | EventType::PackageUpdate
            | EventType::PackageRemove => Self::Package,

            EventType::ServiceStart
            | EventType::ServiceStop
            | EventType::ServiceRestart
            | EventType::ServiceFailed => Self::Service,

            EventType::AppCrash => Self::Application,

            EventType::KernelWarning | EventType::KernelError => Self::Kernel,

            EventType::PermissionDenied => Self::Security,

            EventType::NetworkLinkDown
            | EventType::NetworkLinkUp
            | EventType::NetworkDhcpFailure
            | EventType::NetworkDnsFailure => Self::Network,

            EventType::DiskSpaceWarning
            | EventType::DiskSpaceCritical
            | EventType::DiskInodeExhaustion => Self::Disk,

            EventType::SystemBoot | EventType::SystemShutdown => Self::System,

            EventType::Other => Self::Other,
        }
    }

    /// Returns a human-readable label for this category.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Package => "Package Management",
            Self::Service => "Services",
            Self::Application => "Applications",
            Self::Kernel => "Kernel",
            Self::Security => "Security",
            Self::Network => "Network",
            Self::Disk => "Disk & Storage",
            Self::System => "System",
            Self::Other => "Other",
        }
    }

    /// Returns the Freedesktop icon name for this category.
    #[must_use]
    pub const fn icon_name(&self) -> &'static str {
        match self {
            Self::Package => "package-x-generic-symbolic",
            Self::Service => "system-run-symbolic",
            Self::Application => "application-x-executable-symbolic",
            Self::Kernel => "computer-symbolic",
            Self::Security => "security-high-symbolic",
            Self::Network => "network-wired-symbolic",
            Self::Disk => "drive-harddisk-symbolic",
            Self::System => "computer-symbolic",
            Self::Other => "dialog-information-symbolic",
        }
    }
}

/// Returns true if two event types are in the same category.
#[must_use]
pub fn same_category(a: EventType, b: EventType) -> bool {
    EventCategory::for_event_type(a) == EventCategory::for_event_type(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_categorization() {
        assert_eq!(
            EventCategory::for_event_type(EventType::PackageInstall),
            EventCategory::Package
        );
        assert_eq!(
            EventCategory::for_event_type(EventType::ServiceFailed),
            EventCategory::Service
        );
        assert_eq!(
            EventCategory::for_event_type(EventType::NetworkLinkDown),
            EventCategory::Network
        );
    }

    #[test]
    fn test_same_category() {
        assert!(same_category(EventType::PackageInstall, EventType::PackageUpdate));
        assert!(!same_category(EventType::PackageInstall, EventType::ServiceStart));
    }
}
