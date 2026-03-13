//! Kernel message adapter.
//!
//! Reads kernel messages from dmesg/journal.

use crate::domain::event::{Event, EventType, Evidence, EvidenceSource, Severity};
use crate::infrastructure::adapters::{AdapterError, EventAdapter};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use std::process::Command;
use tracing::debug;

// Static regex patterns for kernel message classification
static ERROR_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)error|fail|fault|crash|panic|oops").expect("valid regex"));
static WARNING_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)warning|warn|deprecated").expect("valid regex"));

/// Adapter for kernel messages.
pub struct KernelAdapter;

impl KernelAdapter {
    /// Creates a new kernel adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    fn classify_message(&self, message: &str) -> Option<(EventType, Severity)> {
        // Check for errors first
        if ERROR_PATTERN.is_match(message) {
            return Some((EventType::KernelError, Severity::Error));
        }

        // Then warnings
        if WARNING_PATTERN.is_match(message) {
            return Some((EventType::KernelWarning, Severity::Warning));
        }

        None
    }
}

impl Default for KernelAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventAdapter for KernelAdapter {
    fn name(&self) -> &'static str {
        "kernel"
    }

    fn is_available(&self) -> bool {
        Command::new("dmesg").arg("--version").output().is_ok()
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        debug!("Reading kernel messages");

        // Use journalctl for kernel messages (more reliable timestamps)
        let since_str = since.format("%Y-%m-%d %H:%M:%S").to_string();

        let output = Command::new("journalctl")
            .args([
                "-k",              // Kernel messages only
                "--since", &since_str,
                "-p", "warning",   // Warning and above
                "--no-pager",
                "-o", "short-iso",
            ])
            .output()
            .map_err(|e| AdapterError::KernelError(e.to_string()))?;

        if !output.status.success() {
            // Fall back to dmesg if journalctl fails
            return self.read_from_dmesg(since);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut events = Vec::new();

        for line in stdout.lines() {
            if line.trim().is_empty() {
                continue;
            }

            // Parse timestamp from journalctl output
            // Format: 2024-01-15T10:30:45+0000 hostname kernel: message
            let parts: Vec<&str> = line.splitn(4, ' ').collect();
            if parts.len() < 4 {
                continue;
            }

            let message = parts[3..].join(" ");
            if let Some((event_type, severity)) = self.classify_message(&message) {
                events.push(
                    Event::new(Utc::now(), event_type, severity, message.clone())
                        .with_evidence(Evidence {
                            source: EvidenceSource::Kernel,
                            cursor: None,
                            file_path: None,
                            raw_content: line.to_string(),
                            line_number: None,
                        }),
                );
            }
        }

        debug!(count = events.len(), "Parsed kernel events");
        Ok(events)
    }
}

impl KernelAdapter {
    fn read_from_dmesg(&self, _since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        let output = Command::new("dmesg")
            .args(["--level=warn,err,crit,alert,emerg", "--time-format=iso"])
            .output()
            .map_err(|e| AdapterError::KernelError(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut events = Vec::new();

        for line in stdout.lines() {
            if let Some((event_type, severity)) = self.classify_message(line) {
                events.push(
                    Event::new(Utc::now(), event_type, severity, line.to_string())
                        .with_evidence(Evidence {
                            source: EvidenceSource::Kernel,
                            cursor: None,
                            file_path: None,
                            raw_content: line.to_string(),
                            line_number: None,
                        }),
                );
            }
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_error() {
        let adapter = KernelAdapter::new();
        
        let result = adapter.classify_message("ata1.00: failed to IDENTIFY");
        assert!(matches!(result, Some((EventType::KernelError, Severity::Error))));
    }

    #[test]
    fn test_classify_warning() {
        let adapter = KernelAdapter::new();
        
        let result = adapter.classify_message("CPU: Deprecated feature detected");
        assert!(matches!(result, Some((EventType::KernelWarning, Severity::Warning))));
    }

    #[test]
    fn test_no_classification_for_normal() {
        let adapter = KernelAdapter::new();
        
        let result = adapter.classify_message("eth0: Link is Up 1000Mbps");
        assert!(result.is_none());
    }
}
