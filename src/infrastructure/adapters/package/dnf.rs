//! DNF package manager adapter.
//!
//! Reads package history from DNF (Fedora, RHEL, CentOS).

use crate::domain::event::{
    Event, EventType, Evidence, EvidenceSource, PackageManagerType, Severity,
};
use crate::infrastructure::adapters::{AdapterError, EventAdapter};
use chrono::{DateTime, NaiveDateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::Path;
use tracing::debug;

const DNF_LOG_PATH: &str = "/var/log/dnf.log";

/// Static regex pattern for parsing DNF log lines.
/// Pattern: 2024-01-15T10:30:45+0000 INFO Upgrade: package-1.0-1.fc39.x86_64
static LINE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4})\s+\w+\s+(Install|Upgrade|Erase|Reinstall):\s+(.+)$")
        .expect("valid regex")
});

/// Adapter for DNF package manager.
pub struct DnfAdapter {
    log_path: &'static str,
}

impl DnfAdapter {
    /// Creates a new DNF adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_path: DNF_LOG_PATH,
        }
    }

    fn parse_log_line(&self, line: &str) -> Option<Event> {
        let caps = LINE_PATTERN.captures(line)?;

        let timestamp_str = caps.get(1)?.as_str();
        let action = caps.get(2)?.as_str();
        let package = caps.get(3)?.as_str().trim();

        // Parse timestamp
        let timestamp = parse_dnf_timestamp(timestamp_str)?;

        let event_type = match action {
            "Install" => EventType::PackageInstall,
            "Upgrade" => EventType::PackageUpdate,
            "Erase" => EventType::PackageRemove,
            "Reinstall" => EventType::PackageUpdate,
            _ => return None,
        };

        let summary = match event_type {
            EventType::PackageInstall => format!("Installed {}", package),
            EventType::PackageUpdate => format!("Updated {}", package),
            EventType::PackageRemove => format!("Removed {}", package),
            _ => format!("{}: {}", action, package),
        };

        // Extract just the package name (without version)
        let pkg_name = package.split('-').next().unwrap_or(package).to_string();

        Some(
            Event::new(timestamp, event_type, Severity::Info, summary)
                .with_package(pkg_name)
                .with_evidence(Evidence {
                    source: EvidenceSource::PackageManager(PackageManagerType::Dnf),
                    cursor: None,
                    file_path: Some(DNF_LOG_PATH.into()),
                    raw_content: line.to_string(),
                    line_number: None,
                }),
        )
    }
}

impl Default for DnfAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for DnfAdapter {
    fn name(&self) -> &'static str {
        "dnf"
    }

    fn is_available(&self) -> bool {
        Path::new(self.log_path).exists()
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        if !self.is_available() {
            return Err(AdapterError::NotAvailable("DNF log not found".to_string()));
        }

        debug!(path = self.log_path, "Reading DNF log");

        let content = fs::read_to_string(self.log_path)?;
        let mut events = Vec::new();

        for line in content.lines() {
            if let Some(event) = self.parse_log_line(line) {
                if event.timestamp >= since {
                    events.push(event);
                }
            }
        }

        debug!(count = events.len(), "Parsed DNF events");
        Ok(events)
    }
}

fn parse_dnf_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // DNF uses format like: 2024-01-15T10:30:45+0000
    // We need to handle the timezone offset
    let naive = NaiveDateTime::parse_from_str(&s[..19], "%Y-%m-%dT%H:%M:%S").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_install_line() {
        let adapter = DnfAdapter::new();
        let line = "2024-01-15T10:30:45+0000 INFO Install: vim-enhanced-9.0.2153-1.fc39.x86_64";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageInstall);
        assert!(event.summary.contains("vim-enhanced"));
    }

    #[test]
    fn test_parse_upgrade_line() {
        let adapter = DnfAdapter::new();
        let line = "2024-01-15T10:30:45+0000 INFO Upgrade: kernel-6.6.9-200.fc39.x86_64";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageUpdate);
    }

    #[test]
    fn test_adapter_name() {
        let adapter = DnfAdapter::new();
        assert_eq!(adapter.name(), "dnf");
    }
}
