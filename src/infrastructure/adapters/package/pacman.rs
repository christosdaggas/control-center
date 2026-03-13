//! Pacman package manager adapter.
//!
//! Reads package history from Pacman (Arch Linux, Manjaro, EndeavourOS).

use crate::domain::event::{
    Event, EventType, Evidence, EvidenceSource, PackageManagerType, Severity,
};
use crate::infrastructure::adapters::{AdapterError, EventAdapter};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::Path;
use tracing::{debug, warn};

const PACMAN_LOG_PATH: &str = "/var/log/pacman.log";

/// Regex pattern for parsing pacman log lines.
/// Format: [YYYY-MM-DDTHH:MM:SS+ZZZZ] [ALPM] installed/upgraded/removed package (version)
static LINE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\[(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{4})\] \[ALPM\] (installed|upgraded|removed|reinstalled) (.+?) \((.+?)\)$"
    ).expect("valid regex")
});

/// Adapter for Pacman package manager.
pub struct PacmanAdapter {
    log_path: &'static str,
}

impl PacmanAdapter {
    /// Creates a new Pacman adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_path: PACMAN_LOG_PATH,
        }
    }

    fn parse_log_line(&self, line: &str) -> Option<Event> {
        let caps = LINE_PATTERN.captures(line)?;

        let timestamp_str = caps.get(1)?.as_str();
        let action = caps.get(2)?.as_str();
        let package = caps.get(3)?.as_str().trim();
        let version = caps.get(4)?.as_str().trim();

        // Parse timestamp (format: 2024-01-15T10:30:45+0000)
        let timestamp = parse_pacman_timestamp(timestamp_str)?;

        let event_type = match action {
            "installed" => EventType::PackageInstall,
            "upgraded" => EventType::PackageUpdate,
            "removed" => EventType::PackageRemove,
            "reinstalled" => EventType::PackageUpdate,
            _ => return None,
        };

        let summary = match event_type {
            EventType::PackageInstall => format!("Installed {} ({})", package, version),
            EventType::PackageUpdate => format!("Updated {} to {}", package, version),
            EventType::PackageRemove => format!("Removed {} ({})", package, version),
            _ => format!("{}: {} ({})", action, package, version),
        };

        Some(
            Event::new(timestamp, event_type, Severity::Info, summary)
                .with_package(package.to_string())
                .with_evidence(Evidence {
                    source: EvidenceSource::PackageManager(PackageManagerType::Rpm), // Use Rpm as placeholder for Pacman
                    cursor: None,
                    file_path: Some(PACMAN_LOG_PATH.into()),
                    raw_content: line.to_string(),
                    line_number: None,
                }),
        )
    }
}

impl Default for PacmanAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for PacmanAdapter {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn is_available(&self) -> bool {
        let available = Path::new(self.log_path).exists();
        if !available {
            debug!(path = self.log_path, "Pacman log not found");
        }
        available
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        if !self.is_available() {
            return Err(AdapterError::NotAvailable("Pacman log not found".to_string()));
        }

        debug!(path = self.log_path, "Reading Pacman log");

        let content = fs::read_to_string(self.log_path).map_err(|e| {
            warn!(error = %e, path = self.log_path, "Failed to read Pacman log");
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

        debug!(count = events.len(), "Parsed Pacman events");
        Ok(events)
    }
}

fn parse_pacman_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // Pacman uses format: 2024-01-15T10:30:45+0000
    DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%z")
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_installed_line() {
        let adapter = PacmanAdapter::new();
        let line = "[2024-01-15T10:30:45+0000] [ALPM] installed vim (9.0.2153-1)";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageInstall);
        assert!(event.summary.contains("vim"));
        assert!(event.summary.contains("9.0.2153-1"));
    }

    #[test]
    fn test_parse_upgraded_line() {
        let adapter = PacmanAdapter::new();
        let line = "[2024-01-15T10:30:45+0000] [ALPM] upgraded linux (6.6.8-1 -> 6.6.9-1)";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageUpdate);
    }

    #[test]
    fn test_parse_removed_line() {
        let adapter = PacmanAdapter::new();
        let line = "[2024-01-15T10:30:45+0000] [ALPM] removed firefox (121.0-1)";

        let event = adapter.parse_log_line(line);
        assert!(event.is_some());

        let event = event.unwrap();
        assert_eq!(event.event_type, EventType::PackageRemove);
    }

    #[test]
    fn test_adapter_name() {
        let adapter = PacmanAdapter::new();
        assert_eq!(adapter.name(), "pacman");
    }

    #[test]
    fn test_parse_timestamp() {
        let ts = parse_pacman_timestamp("2024-01-15T10:30:45+0000");
        assert!(ts.is_some());
    }
}
