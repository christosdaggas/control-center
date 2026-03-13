//! Systemd journal adapter.
//!
//! Reads events from the systemd journal and normalizes them.

use super::{AdapterError, EventAdapter};
use crate::domain::event::{Event, EventType, Evidence, Severity};
use chrono::{DateTime, TimeZone, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Command;
use tracing::{debug, warn};

// Static regex patterns for parsing journal entries
static SERVICE_START_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Started (.+)\.").expect("valid regex"));
static SERVICE_STOP_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Stopped (.+)\.").expect("valid regex"));
static SERVICE_FAILED_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(.+) failed|Failed to start (.+)").expect("valid regex"));
static SELINUX_DENIAL_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"SELinux is preventing|avc:\s*denied").expect("valid regex"));
static APPARMOR_DENIAL_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"apparmor="DENIED""#).expect("valid regex"));

/// Adapter for reading from systemd journal.
pub struct JournaldAdapter;

impl JournaldAdapter {
    /// Creates a new journald adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parses a journal entry into an Event.
    fn parse_entry(&self, entry: &JournalEntry) -> Option<Event> {
        let message = &entry.message;

        // Service events
        if let Some(caps) = SERVICE_START_PATTERN.captures(message) {
            let service_name = caps.get(1).map_or("unknown", |m| m.as_str());
            return Some(
                Event::new(
                    entry.timestamp,
                    EventType::ServiceStart,
                    Severity::Info,
                    format!("{} started", service_name),
                )
                .with_service(entry.unit.clone().unwrap_or_else(|| service_name.to_string()))
                .with_evidence(Evidence::from_journald(
                    entry.cursor.clone(),
                    entry.message.clone(),
                )),
            );
        }

        if let Some(caps) = SERVICE_STOP_PATTERN.captures(message) {
            let service_name = caps.get(1).map_or("unknown", |m| m.as_str());
            return Some(
                Event::new(
                    entry.timestamp,
                    EventType::ServiceStop,
                    Severity::Info,
                    format!("{} stopped", service_name),
                )
                .with_service(entry.unit.clone().unwrap_or_else(|| service_name.to_string()))
                .with_evidence(Evidence::from_journald(
                    entry.cursor.clone(),
                    entry.message.clone(),
                )),
            );
        }

        if SERVICE_FAILED_PATTERN.is_match(message) {
            return Some(
                Event::new(
                    entry.timestamp,
                    EventType::ServiceFailed,
                    Severity::Error,
                    message.clone(),
                )
                .with_service(entry.unit.clone().unwrap_or_default())
                .with_evidence(Evidence::from_journald(
                    entry.cursor.clone(),
                    entry.message.clone(),
                )),
            );
        }

        // SELinux/AppArmor denials
        if SELINUX_DENIAL_PATTERN.is_match(message)
            || APPARMOR_DENIAL_PATTERN.is_match(message)
        {
            return Some(
                Event::new(
                    entry.timestamp,
                    EventType::PermissionDenied,
                    Severity::Warning,
                    "Permission denied by security policy".to_string(),
                )
                .with_details(message.clone())
                .with_evidence(Evidence::from_journald(
                    entry.cursor.clone(),
                    entry.message.clone(),
                )),
            );
        }

        // Map priority to severity for remaining events
        // Journal priorities: 0=emerg, 1=alert, 2=crit, 3=err, 4=warning, 5=notice, 6=info, 7=debug
        let severity = match entry.priority {
            0 | 1 => Severity::Critical,
            2 | 3 => Severity::Error,
            4 => Severity::Warning,
            _ => Severity::Info,
        };

        // Skip debug messages
        if entry.priority > 6 {
            return None;
        }

        let event_type = if entry.transport == "kernel" {
            if severity >= Severity::Warning {
                EventType::KernelWarning
            } else {
                EventType::Other
            }
        } else {
            EventType::Other
        };

        Some(
            Event::new(entry.timestamp, event_type, severity, message.clone())
                .with_evidence(Evidence::from_journald(
                    entry.cursor.clone(),
                    entry.message.clone(),
                )),
        )
    }
}

impl Default for JournaldAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for JournaldAdapter {
    fn name(&self) -> &'static str {
        "journald"
    }

    fn is_available(&self) -> bool {
        // Check if journalctl is available
        Command::new("journalctl")
            .arg("--version")
            .output()
            .is_ok()
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

        debug!(since = %since_str, "Reading journal entries");

        // Use journalctl with JSON output for reliable parsing
        let output = Command::new("journalctl")
            .args([
                "--since",
                &since_str,
                "--output",
                "json",
                "--no-pager",
                "-p",
                "info", // Info and above to get more events (0-6)
            ])
            .output()
            .map_err(|e| AdapterError::JournalError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::JournalError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut events = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(json) => {
                    if let Some(entry) = parse_journal_json(&json) {
                        if let Some(event) = self.parse_entry(&entry) {
                            events.push(event);
                        }
                    }
                }
                Err(e) => {
                    warn!(line = line, error = %e, "Failed to parse journal entry");
                }
            }
        }

        debug!(count = events.len(), "Parsed journal events");
        Ok(events)
    }
}

/// Parsed journal entry structure.
struct JournalEntry {
    timestamp: DateTime<Utc>,
    message: String,
    cursor: String,
    unit: Option<String>,
    priority: u8,
    transport: String,
}

fn parse_journal_json(json: &serde_json::Value) -> Option<JournalEntry> {
    let message = json.get("MESSAGE")?.as_str()?.to_string();

    let timestamp_us = json
        .get("__REALTIME_TIMESTAMP")?
        .as_str()?
        .parse::<i64>()
        .ok()?;
    let timestamp = Utc.timestamp_micros(timestamp_us).single()?;

    let cursor = json
        .get("__CURSOR")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let unit = json
        .get("_SYSTEMD_UNIT")
        .or_else(|| json.get("UNIT"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let priority = json
        .get("PRIORITY")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(6); // default to info

    let transport = json
        .get("_TRANSPORT")
        .and_then(|v| v.as_str())
        .unwrap_or("journal")
        .to_string();

    Some(JournalEntry {
        timestamp,
        message,
        cursor,
        unit,
        priority,
        transport,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = JournaldAdapter::new();
        assert_eq!(adapter.name(), "journald");
    }

    #[test]
    fn test_service_start_pattern() {
        assert!(SERVICE_START_PATTERN.is_match("Started nginx.service."));
    }

    #[test]
    fn test_selinux_pattern() {
        assert!(SELINUX_DENIAL_PATTERN.is_match("SELinux is preventing nginx from read access"));
        assert!(SELINUX_DENIAL_PATTERN.is_match("avc:  denied  { read }"));
    }
}
