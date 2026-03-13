//! Custom log file adapter.
//!
//! Reads events from user-configured log files using regex patterns.

use crate::domain::event::{Event, EventType, Evidence, EvidenceSource, Severity};
use crate::infrastructure::adapters::{AdapterError, EventAdapter};
use chrono::{DateTime, NaiveDateTime, Utc};
use regex::Regex;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Configuration for a custom log source.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomLogConfig {
    /// Display name for this source.
    pub name: String,
    /// Path to the log file.
    pub path: PathBuf,
    /// Regex pattern with named groups: `timestamp`, `level`, `message`.
    pub pattern: String,
    /// strftime format for parsing the timestamp group.
    pub timestamp_format: String,
    /// Whether this source is enabled.
    pub enabled: bool,
}

impl Default for CustomLogConfig {
    fn default() -> Self {
        Self {
            name: "Custom Log".to_string(),
            path: PathBuf::from("/var/log/syslog"),
            pattern: r"^(?P<timestamp>\w{3}\s+\d+\s+\d{2}:\d{2}:\d{2})\s+\S+\s+(?P<service>\S+?)(?:\[\d+\])?:\s+(?P<message>.+)$".to_string(),
            timestamp_format: "%b %d %H:%M:%S".to_string(),
            enabled: false,
        }
    }
}

/// Adapter for reading events from custom log files.
pub struct CustomLogAdapter {
    configs: Vec<CustomLogConfig>,
}

impl CustomLogAdapter {
    /// Creates a new custom log adapter with the given configurations.
    #[must_use]
    pub fn new(configs: Vec<CustomLogConfig>) -> Self {
        Self { configs }
    }

    /// Parses a single log file using the given config.
    fn parse_file(
        &self,
        config: &CustomLogConfig,
        since: DateTime<Utc>,
    ) -> Result<Vec<Event>, AdapterError> {
        let path = &config.path;
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| AdapterError::IoError(e))?;

        let pattern = Regex::new(&config.pattern)
            .map_err(|e| AdapterError::PackageHistoryError(format!("Invalid regex: {e}")))?;

        let mut events = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }

            let Some(caps) = pattern.captures(line) else {
                continue;
            };

            // Parse timestamp
            let timestamp = caps.name("timestamp")
                .and_then(|m| {
                    NaiveDateTime::parse_from_str(m.as_str(), &config.timestamp_format)
                        .ok()
                        .map(|ndt| ndt.and_utc())
                        .or_else(|| {
                            // Try with current year prepended for syslog-style timestamps
                            let with_year = format!("{} {}", Utc::now().format("%Y"), m.as_str());
                            NaiveDateTime::parse_from_str(&with_year, &format!("%Y {}", config.timestamp_format))
                                .ok()
                                .map(|ndt| ndt.and_utc())
                        })
                })
                .unwrap_or(Utc::now());

            if timestamp < since {
                continue;
            }

            // Parse severity from level group or message content
            let severity = caps.name("level")
                .map(|m| match m.as_str().to_lowercase().as_str() {
                    "error" | "err" | "crit" | "critical" | "alert" | "emerg" => Severity::Error,
                    "warn" | "warning" => Severity::Warning,
                    _ => Severity::Info,
                })
                .unwrap_or_else(|| classify_severity(line));

            let message = caps.name("message")
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| line.to_string());

            let service = caps.name("service").map(|m| m.as_str().to_string());

            let mut event = Event::new(
                timestamp,
                EventType::Other,
                severity,
                format!("[{}] {}", config.name, truncate(&message, 120)),
            )
            .with_evidence(Evidence {
                source: EvidenceSource::Syslog,
                cursor: None,
                file_path: Some(config.path.clone()),
                raw_content: line.to_string(),
                line_number: Some(line_num as u64 + 1),
            });

            if let Some(svc) = service {
                event = event.with_service(svc);
            }

            events.push(event);
        }

        debug!(source = %config.name, count = events.len(), "Parsed custom log events");
        Ok(events)
    }
}

impl EventAdapter for CustomLogAdapter {
    fn name(&self) -> &'static str {
        "custom-log"
    }

    fn is_available(&self) -> bool {
        self.configs.iter().any(|c| c.enabled && c.path.exists())
    }

    fn read_since(&self, since: DateTime<Utc>) -> Result<Vec<Event>, AdapterError> {
        let mut all_events = Vec::new();

        for config in &self.configs {
            if !config.enabled {
                continue;
            }

            match self.parse_file(config, since) {
                Ok(events) => all_events.extend(events),
                Err(e) => {
                    warn!(source = %config.name, error = %e, "Failed to read custom log");
                }
            }
        }

        Ok(all_events)
    }
}

/// Classify severity based on message content.
fn classify_severity(message: &str) -> Severity {
    let lower = message.to_lowercase();
    if lower.contains("error") || lower.contains("fail") || lower.contains("crit") {
        Severity::Error
    } else if lower.contains("warn") {
        Severity::Warning
    } else {
        Severity::Info
    }
}

/// Truncate a string to max length with ellipsis.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_classify_severity() {
        assert_eq!(classify_severity("Something error happened"), Severity::Error);
        assert_eq!(classify_severity("warning: low disk"), Severity::Warning);
        assert_eq!(classify_severity("started service"), Severity::Info);
        assert_eq!(classify_severity("critical failure"), Severity::Error);
        assert_eq!(classify_severity("operation failed"), Severity::Error);
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world this is long", 10), "hello w...");
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("abcd", 3), "...");
    }

    #[test]
    fn test_default_config() {
        let config = CustomLogConfig::default();
        assert_eq!(config.name, "Custom Log");
        assert!(!config.enabled);
        assert!(!config.timestamp_format.is_empty());
        // Pattern should compile
        assert!(Regex::new(&config.pattern).is_ok());
    }

    #[test]
    fn test_adapter_name() {
        let adapter = CustomLogAdapter::new(vec![]);
        assert_eq!(adapter.name(), "custom-log");
    }

    #[test]
    fn test_disabled_configs_skipped() {
        let config = CustomLogConfig {
            enabled: false,
            ..CustomLogConfig::default()
        };
        let adapter = CustomLogAdapter::new(vec![config]);
        assert!(!adapter.is_available());
        let events = adapter.read_since(Utc::now() - chrono::Duration::hours(1)).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_simple_log_file() {
        let mut tmp = NamedTempFile::new().unwrap();
        let now = Utc::now();
        let ts = now.format("%Y-%m-%d %H:%M:%S").to_string();
        writeln!(tmp, "{} ERROR Something went wrong", ts).unwrap();
        writeln!(tmp, "{} INFO Service started", ts).unwrap();

        let config = CustomLogConfig {
            name: "TestLog".to_string(),
            path: tmp.path().to_path_buf(),
            pattern: r"^(?P<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})\s+(?P<level>\w+)\s+(?P<message>.+)$".to_string(),
            timestamp_format: "%Y-%m-%d %H:%M:%S".to_string(),
            enabled: true,
        };

        let adapter = CustomLogAdapter::new(vec![config]);
        let since = now - chrono::Duration::hours(1);
        let events = adapter.read_since(since).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events[0].summary.contains("[TestLog]"));
    }

    #[test]
    fn test_nonexistent_file_returns_empty() {
        let config = CustomLogConfig {
            name: "Missing".to_string(),
            path: PathBuf::from("/nonexistent/path/log.txt"),
            pattern: r"(?P<timestamp>\S+) (?P<message>.+)".to_string(),
            timestamp_format: "%Y-%m-%d".to_string(),
            enabled: true,
        };

        let adapter = CustomLogAdapter::new(vec![config]);
        let events = adapter.read_since(Utc::now() - chrono::Duration::hours(1)).unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_invalid_regex_returns_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        writeln!(tmp, "some log line").unwrap();

        let config = CustomLogConfig {
            name: "BadRegex".to_string(),
            path: tmp.path().to_path_buf(),
            pattern: r"[invalid(".to_string(),
            timestamp_format: "%Y".to_string(),
            enabled: true,
        };

        let adapter = CustomLogAdapter::new(vec![config]);
        // Should not panic; individual errors are warned and skipped
        let result = adapter.read_since(Utc::now() - chrono::Duration::hours(1));
        assert!(result.is_ok());
    }
}
