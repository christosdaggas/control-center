//! Export use case.
//!
//! Exports events and correlation groups to JSON, CSV, or Markdown.

use crate::domain::correlation::CorrelationGroup;
use crate::domain::event::Event;
use std::fmt::Write as FmtWrite;
use thiserror::Error;

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// JSON format.
    Json,
    /// CSV format.
    Csv,
    /// Markdown format.
    Markdown,
}

impl ExportFormat {
    /// Returns the file extension for this format.
    #[must_use]
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Csv => "csv",
            Self::Markdown => "md",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub const fn mime_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Csv => "text/csv",
            Self::Markdown => "text/markdown",
        }
    }
}

/// Errors from export operations.
#[derive(Debug, Error)]
pub enum ExportError {
    /// Serialization failure.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Formatting failure.
    #[error("Formatting error: {0}")]
    Format(#[from] std::fmt::Error),
}

/// Exports events to a string in the specified format.
///
/// # Errors
///
/// Returns `ExportError` if serialization or formatting fails.
pub fn export_events(
    events: &[Event],
    groups: &[CorrelationGroup],
    format: ExportFormat,
) -> Result<String, ExportError> {
    match format {
        ExportFormat::Json => export_json(events, groups),
        ExportFormat::Csv => export_csv(events),
        ExportFormat::Markdown => export_markdown(events, groups),
    }
}

/// Exports events and groups to JSON.
fn export_json(
    events: &[Event],
    groups: &[CorrelationGroup],
) -> Result<String, ExportError> {
    #[derive(serde::Serialize)]
    struct ExportData<'a> {
        exported_at: String,
        event_count: usize,
        group_count: usize,
        events: &'a [Event],
        correlation_groups: &'a [CorrelationGroup],
    }

    let data = ExportData {
        exported_at: chrono::Utc::now().to_rfc3339(),
        event_count: events.len(),
        group_count: groups.len(),
        events,
        correlation_groups: groups,
    };

    serde_json::to_string_pretty(&data)
        .map_err(|e| ExportError::Serialization(e.to_string()))
}

/// Exports events to CSV.
fn export_csv(events: &[Event]) -> Result<String, ExportError> {
    let mut out = String::new();
    writeln!(out, "timestamp,severity,event_type,summary,service,package")?;

    for event in events {
        writeln!(
            out,
            "{},{},{},\"{}\",{},{}",
            event.timestamp.to_rfc3339(),
            event.severity.label(),
            event.event_type.label(),
            event.summary.replace('"', "\"\""),
            event.service.as_deref().unwrap_or(""),
            event.package.as_deref().unwrap_or(""),
        )?;
    }

    Ok(out)
}

/// Exports events and groups to Markdown.
fn export_markdown(
    events: &[Event],
    groups: &[CorrelationGroup],
) -> Result<String, ExportError> {
    let mut out = String::new();

    writeln!(out, "# Control Center Event Export")?;
    writeln!(out)?;
    writeln!(
        out,
        "Exported at: {}",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    )?;
    writeln!(out)?;

    if !groups.is_empty() {
        writeln!(out, "## Correlated Event Groups")?;
        writeln!(out)?;
        for group in groups {
            writeln!(out, "### {}", group.rule_title)?;
            writeln!(out)?;
            writeln!(out, "- **Rule**: {}", group.rule_id)?;
            writeln!(out, "- **Confidence**: {}%", group.confidence)?;
            writeln!(out, "- **Events**: {}", group.events.len())?;
            writeln!(out, "- **Explanation**: {}", group.explanation)?;
            writeln!(out)?;
        }
    }

    writeln!(out, "## Events ({} total)", events.len())?;
    writeln!(out)?;
    writeln!(out, "| Time | Severity | Type | Summary |")?;
    writeln!(out, "|------|----------|------|---------|")?;

    for event in events {
        writeln!(
            out,
            "| {} | {} | {} | {} |",
            event.timestamp.format("%Y-%m-%d %H:%M:%S"),
            event.severity.label(),
            event.event_type.label(),
            event.summary,
        )?;
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{EventType, Severity};
    use chrono::Utc;

    fn sample_events() -> Vec<Event> {
        vec![
            Event::new(
                Utc::now(),
                EventType::ServiceFailed,
                Severity::Error,
                "nginx.service failed".to_string(),
            )
            .with_service("nginx.service"),
            Event::new(
                Utc::now(),
                EventType::PackageUpdate,
                Severity::Info,
                "Updated vim to 9.1".to_string(),
            )
            .with_package("vim"),
        ]
    }

    #[test]
    fn test_export_json() {
        let events = sample_events();
        let result = export_events(&events, &[], ExportFormat::Json);
        assert!(result.is_ok());
        let json = result.unwrap();
        assert!(json.contains("event_count"));
        assert!(json.contains("nginx.service failed"));
    }

    #[test]
    fn test_export_csv() {
        let events = sample_events();
        let result = export_events(&events, &[], ExportFormat::Csv);
        assert!(result.is_ok());
        let csv = result.unwrap();
        assert!(csv.starts_with("timestamp,severity"));
        assert!(csv.contains("nginx.service failed"));
    }

    #[test]
    fn test_export_markdown() {
        let events = sample_events();
        let result = export_events(&events, &[], ExportFormat::Markdown);
        assert!(result.is_ok());
        let md = result.unwrap();
        assert!(md.contains("# Control Center Event Export"));
        assert!(md.contains("nginx.service failed"));
    }
}
