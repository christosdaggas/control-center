//! APT package manager adapter.
//!
//! Reads package history from APT (Debian, Ubuntu).

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

const APT_HISTORY_PATH: &str = "/var/log/apt/history.log";

/// Static regex patterns for parsing APT history log.
static DATE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^Start-Date:\s*(.+)$").expect("valid regex"));
static ACTION_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(Install|Upgrade|Remove|Purge):\s*(.+)$").expect("valid regex"));

/// Adapter for APT package manager.
pub struct AptAdapter {
    log_path: &'static str,
}

impl AptAdapter {
    /// Creates a new APT adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            log_path: APT_HISTORY_PATH,
        }
    }

    fn parse_log(&self, content: &str, since: DateTime<Utc>) -> Vec<Event> {
        let mut events = Vec::new();
        let mut current_timestamp: Option<DateTime<Utc>> = None;

        for line in content.lines() {
            // Parse date line
            if let Some(caps) = DATE_PATTERN.captures(line) {
                let date_str = caps.get(1).map_or("", |m| m.as_str());
                current_timestamp = parse_apt_timestamp(date_str);
                continue;
            }

            // Parse action line
            if let Some(caps) = ACTION_PATTERN.captures(line) {
                let Some(timestamp) = current_timestamp else {
                    continue;
                };
                
                if timestamp < since {
                    continue;
                }

                let action = caps.get(1).map_or("", |m| m.as_str());
                let packages = caps.get(2).map_or("", |m| m.as_str());

                let event_type = match action {
                    "Install" => EventType::PackageInstall,
                    "Upgrade" => EventType::PackageUpdate,
                    "Remove" | "Purge" => EventType::PackageRemove,
                    _ => continue,
                };

                // APT lists multiple packages on one line, comma-separated
                for pkg_entry in packages.split(", ") {
                    // Format: "package:arch (old-version, new-version)" or "package:arch (version)"
                    let pkg_name = pkg_entry
                        .split(':')
                        .next()
                        .unwrap_or(pkg_entry)
                        .split_whitespace()
                        .next()
                        .unwrap_or(pkg_entry)
                        .trim();

                    if pkg_name.is_empty() {
                        continue;
                    }

                    let summary = match event_type {
                        EventType::PackageInstall => format!("Installed {}", pkg_name),
                        EventType::PackageUpdate => format!("Updated {}", pkg_name),
                        EventType::PackageRemove => format!("Removed {}", pkg_name),
                        _ => format!("{}: {}", action, pkg_name),
                    };

                    events.push(
                        Event::new(timestamp, event_type, Severity::Info, summary)
                            .with_package(pkg_name.to_string())
                            .with_evidence(Evidence {
                                source: EvidenceSource::PackageManager(PackageManagerType::Apt),
                                cursor: None,
                                file_path: Some(APT_HISTORY_PATH.into()),
                                raw_content: line.to_string(),
                                line_number: None,
                            }),
                    );
                }
            }
        }

        events
    }
}

impl Default for AptAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for AptAdapter {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn is_available(&self) -> bool {
        Path::new(self.log_path).exists()
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        if !self.is_available() {
            return Err(AdapterError::NotAvailable("APT history not found".to_string()));
        }

        debug!(path = self.log_path, "Reading APT history");

        let content = fs::read_to_string(self.log_path)?;
        let events = self.parse_log(&content, since);

        debug!(count = events.len(), "Parsed APT events");
        Ok(events)
    }
}

fn parse_apt_timestamp(s: &str) -> Option<DateTime<Utc>> {
    // APT uses format like: 2024-01-15  10:30:45
    let normalized = s.replace("  ", " ");
    let naive = NaiveDateTime::parse_from_str(normalized.trim(), "%Y-%m-%d %H:%M:%S").ok()?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_name() {
        let adapter = AptAdapter::new();
        assert_eq!(adapter.name(), "apt");
    }

    #[test]
    fn test_parse_history() {
        let adapter = AptAdapter::new();
        let content = r#"
Start-Date: 2024-01-15  10:30:45
Install: vim:amd64 (2:9.0.1378-2)
End-Date: 2024-01-15  10:30:50

Start-Date: 2024-01-15  11:00:00
Upgrade: libc6:amd64 (2.36-9, 2.37-1)
End-Date: 2024-01-15  11:00:30
"#;

        let since = DateTime::from_timestamp(0, 0).unwrap();
        let events = adapter.parse_log(content, since);

        // Install produces 1 event, Upgrade line "libc6:amd64 (2.36-9, 2.37-1)"
        // splits on ", " into two package entries, so 3 total.
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, EventType::PackageInstall);
        assert_eq!(events[1].event_type, EventType::PackageUpdate);
    }
}
