//! Flatpak event source adapter.
//!
//! Reads Flatpak install/update/uninstall history and normalizes into events.

use crate::domain::event::{
    Event, EventType, Evidence, EvidenceSource, PackageManagerType, Severity,
};
use crate::infrastructure::adapters::{AdapterError, EventAdapter};
use chrono::{DateTime, NaiveDateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Command;
use tracing::debug;

/// Regex for parsing `flatpak history` output.
/// Format: "2024-01-15 10:30:45  install  com.example.App  stable  flathub"
static HISTORY_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"^\s*(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2})\s+(install|update|uninstall)\s+(\S+)\s+(\S+)\s+(\S+)",
    )
    .expect("valid regex")
});

/// Adapter for Flatpak package manager events.
pub struct FlatpakAdapter;

impl FlatpakAdapter {
    /// Creates a new Flatpak adapter.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for FlatpakAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for FlatpakAdapter {
    fn name(&self) -> &'static str {
        "flatpak"
    }

    fn is_available(&self) -> bool {
        Command::new("flatpak")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        debug!("Reading Flatpak history");

        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

        let output = Command::new("flatpak")
            .args(["history", "--columns=time,change,ref,branch,remote", "--since", &since_str])
            .output()
            .map_err(|e| AdapterError::PackageHistoryError(format!("Flatpak: {e}")))?;

        if !output.status.success() {
            // Flatpak history may not be available on older versions
            debug!("Flatpak history command not available, trying alternative");
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut events = Vec::new();

        for line in stdout.lines() {
            if let Some(caps) = HISTORY_PATTERN.captures(line) {
                let timestamp_str = caps.get(1).map_or("", |m| m.as_str());
                let action = caps.get(2).map_or("", |m| m.as_str());
                let app_ref = caps.get(3).map_or("", |m| m.as_str());
                let _branch = caps.get(4).map_or("", |m| m.as_str());
                let _remote = caps.get(5).map_or("", |m| m.as_str());

                let timestamp = NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|ndt| ndt.and_utc());

                let Some(timestamp) = timestamp else {
                    continue;
                };

                if timestamp < since {
                    continue;
                }

                let event_type = match action {
                    "install" => EventType::PackageInstall,
                    "update" => EventType::PackageUpdate,
                    "uninstall" => EventType::PackageRemove,
                    _ => continue,
                };

                // Extract readable name from ref (e.g., "app/com.example.App/x86_64/stable" → "com.example.App")
                let app_name = app_ref
                    .split('/')
                    .nth(1)
                    .unwrap_or(app_ref)
                    .to_string();

                let summary = match event_type {
                    EventType::PackageInstall => format!("Flatpak: Installed {app_name}"),
                    EventType::PackageUpdate => format!("Flatpak: Updated {app_name}"),
                    EventType::PackageRemove => format!("Flatpak: Removed {app_name}"),
                    _ => format!("Flatpak: {action} {app_name}"),
                };

                events.push(
                    Event::new(timestamp, event_type, Severity::Info, summary)
                        .with_package(app_name)
                        .with_evidence(Evidence {
                            source: EvidenceSource::PackageManager(PackageManagerType::Flatpak),
                            cursor: None,
                            file_path: None,
                            raw_content: line.to_string(),
                            line_number: None,
                        }),
                );
            }
        }

        debug!(count = events.len(), "Parsed Flatpak events");
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatpak_adapter_name() {
        let adapter = FlatpakAdapter::new();
        assert_eq!(adapter.name(), "flatpak");
    }

    #[test]
    fn test_default_adapter() {
        let adapter = FlatpakAdapter::default();
        assert_eq!(adapter.name(), "flatpak");
    }

    #[test]
    fn test_history_pattern_captures() {
        let line = "2024-01-15 10:30:45  install  app/com.example.App/x86_64/stable  stable  flathub";
        let caps = HISTORY_PATTERN.captures(line);
        assert!(caps.is_some());
        let caps = caps.unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "2024-01-15 10:30:45");
        assert_eq!(caps.get(2).unwrap().as_str(), "install");
        assert_eq!(caps.get(3).unwrap().as_str(), "app/com.example.App/x86_64/stable");
        assert_eq!(caps.get(4).unwrap().as_str(), "stable");
        assert_eq!(caps.get(5).unwrap().as_str(), "flathub");
    }

    #[test]
    fn test_history_pattern_update() {
        let line = "2024-06-01 08:00:00  update  com.spotify.Client  stable  flathub";
        let caps = HISTORY_PATTERN.captures(line);
        assert!(caps.is_some());
        assert_eq!(caps.unwrap().get(2).unwrap().as_str(), "update");
    }

    #[test]
    fn test_history_pattern_uninstall() {
        let line = "2024-03-20 14:15:30  uninstall  org.gnome.Calculator  stable  flathub";
        let caps = HISTORY_PATTERN.captures(line);
        assert!(caps.is_some());
        assert_eq!(caps.unwrap().get(2).unwrap().as_str(), "uninstall");
    }

    #[test]
    fn test_history_pattern_no_match() {
        let line = "This is not a flatpak history line";
        assert!(HISTORY_PATTERN.captures(line).is_none());
    }

    #[test]
    fn test_history_pattern_empty_line() {
        assert!(HISTORY_PATTERN.captures("").is_none());
    }
}
