//! Zypper package manager adapter.
//!
//! Reads package history from Zypper (openSUSE, SLES).

use crate::domain::event::{
    Event, EventType, Evidence, EvidenceSource, PackageManagerType, Severity,
};
use crate::infrastructure::adapters::{AdapterError, EventAdapter};
use chrono::{DateTime, NaiveDateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

const ZYPPER_HISTORY_PATH: &str = "/var/log/zypp/history";

/// Regex pattern for parsing zypper history lines.
/// Format: 2024-01-15 10:30:45|install|package|version|arch|user|repo|hash
/// Or: 2024-01-15 10:30:45|remove|package|version|arch|
static LINE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\|(\w+)\|([^|]+)\|([^|]*)\|").expect("valid regex")
});

/// Adapter for Zypper package manager.
pub struct ZypperAdapter {
    log_path: &'static str,
}

impl ZypperAdapter {
    /// Creates a new Zypper adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_path: ZYPPER_HISTORY_PATH,
        }
    }

    fn parse_log_line(&self, line: &str) -> Option<Event> {
        // Skip comment lines
        if line.starts_with('#') || line.trim().is_empty() {
            return None;
        }

        let caps = LINE_PATTERN.captures(line)?;

        let timestamp_str = caps.get(1)?.as_str();
        let action = caps.get(2)?.as_str();
        let package = caps.get(3)?.as_str().trim();
        let version = caps.get(4)?.as_str().trim();

        // Parse timestamp
        let timestamp = parse_zypper_timestamp(timestamp_str)?;

        let event_type = match action.to_lowercase().as_str() {
            "install" => EventType::PackageInstall,
            "upgrade" => EventType::PackageUpdate,
            "remove" => EventType::PackageRemove,
            "reinstall" => EventType::PackageUpdate,
            _ => return None,
        };

        let summary = match event_type {
            EventType::PackageInstall => {
                if version.is_empty() {
                    format!("Installed {}", package)
                } else {
                    format!("Installed {} ({})", package, version)
                }
            }
            EventType::PackageUpdate => format!("Updated {} to {}", package, version),
            EventType::PackageRemove => format!("Removed {}", package),
            _ => format!("{}: {}", action, package),
        };

        Some(
            Event::new(timestamp, event_type, Severity::Info, summary)
                .with_package(package.to_string())
                .with_evidence(Evidence {
                    source: EvidenceSource::PackageManager(PackageManagerType::Rpm), // Use Rpm as placeholder
                    cursor: None,
                    file_path: Some(ZYPPER_HISTORY_PATH.into()),
                    raw_content: line.to_string(),
                    line_number: None,
                }),
        )
    }
}

impl Default for ZypperAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for ZypperAdapter {
    fn name(&self) -> &'static str {
        "zypper"
    }

    fn is_available(&self) -> bool {
        let available = Path::new(self.log_path).exists();
        if !available {
            debug!(path = self.log_path, "Zypper history not found");
        }
        available
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        if !self.is_available() {
            return Err(AdapterError::NotAvailable(
                "Zypper history not found".to_string(),
            ));
        }

        debug!(path = self.log_path, "Reading Zypper history");

        let content = fs::read_to_string(self.log_path).map_err(|e| {
            warn!(error = %e, path = self.log_path, "Failed to read Zypper history");
            AdapterError::IoError(e)
        })?;

        let mut events = Vec::new();

        for line in content.lines() {
            if let Some(event) = self.parse_log_line(line) {
                if event.timestamp >= since {
                    events.push(event);
                }
            }
        }

        debug!(count = events.len(), "Parsed Zypper events");
        Ok(events)
    }
}

fn parse_zypper_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // Zypper uses format: 2024-01-15 10:30:45
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|naive| DateTime::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_install_line() {
        let adapter = ZypperAdapter::new();
        let line = "2024-01-15 10:30:45|install|vim|9.0.2153|x86_64|root|repo-oss|abc123";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageInstall);
        assert!(event.summary.contains("vim"));
    }

    #[test]
    fn test_parse_remove_line() {
        let adapter = ZypperAdapter::new();
        let line = "2024-01-15 10:30:45|remove|firefox|121.0|x86_64|";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageRemove);
    }

    #[test]
    fn test_skip_comment_lines() {
        let adapter = ZypperAdapter::new();
        let line = "# This is a comment";

        let event = adapter.parse_log_line(line);
        assert!(event.is_none());
    }

    #[test]
    fn test_adapter_name() {
        let adapter = ZypperAdapter::new();
        assert_eq!(adapter.name(), "zypper");
    }

    #[test]
    fn test_parse_timestamp() {
        let ts = parse_zypper_timestamp("2024-01-15 10:30:45");
        assert!(ts.is_some());
    }
}
