//! Human-readable narrative generation.
//!
//! Converts events and correlation groups into explanatory text
//! that tells the story of what happened on the system.

use crate::domain::correlation::rule::CorrelationGroup;
use crate::domain::event::{Event, EventType, Severity};
use chrono::{DateTime, Local, Utc};

/// A human-readable summary of an event or group.
#[derive(Debug, Clone)]
pub struct NarrativeSummary {
    /// Short headline (one line).
    pub headline: String,

    /// Detailed explanation (multiple lines).
    pub details: String,

    /// Suggested action if any.
    pub suggested_action: Option<String>,
}

/// Generates a narrative summary for a single event.
#[must_use]
pub fn summarize_event(event: &Event) -> NarrativeSummary {
    let headline = generate_event_headline(event);
    let details = generate_event_details(event);
    let suggested_action = generate_suggested_action(event);

    NarrativeSummary {
        headline,
        details,
        suggested_action,
    }
}

/// Generates a narrative summary for a correlation group.
#[must_use]
pub fn summarize_group(group: &CorrelationGroup) -> NarrativeSummary {
    let primary = group.primary_event();

    let headline = format!(
        "{}: {}",
        group.rule_title,
        primary.map_or("Related events detected", |e| e.summary.as_str())
    );

    let mut details = group.explanation.clone();

    // Add timeline of events
    details.push_str("\n\nTimeline:\n");
    for event in &group.events {
        let time = format_timestamp(event.timestamp);
        let is_primary = Some(event.id) == primary.map(|e| e.id);
        let marker = if is_primary { "→" } else { "  " };
        details.push_str(&format!("{} {} {}\n", marker, time, event.summary));
    }

    let suggested_action = primary.and_then(|e| generate_suggested_action(e));

    NarrativeSummary {
        headline,
        details,
        suggested_action,
    }
}

fn generate_event_headline(event: &Event) -> String {
    let time = format_timestamp(event.timestamp);
    let severity_icon = match event.severity {
        Severity::Info => "ℹ️",
        Severity::Warning => "⚠️",
        Severity::Error => "❌",
        Severity::Critical => "🔴",
    };

    format!("{} {} {}", severity_icon, time, event.summary)
}

fn generate_event_details(event: &Event) -> String {
    let mut details = Vec::new();

    // Add type-specific details
    match event.event_type {
        EventType::PackageInstall | EventType::PackageUpdate | EventType::PackageRemove => {
            if let Some(pkg) = &event.package {
                details.push(format!("Package: {}", pkg));
            }
        }
        EventType::ServiceStart
        | EventType::ServiceStop
        | EventType::ServiceRestart
        | EventType::ServiceFailed => {
            if let Some(svc) = &event.service {
                details.push(format!("Service: {}", svc));
            }
        }
        _ => {}
    }

    // Add custom details if present
    if let Some(d) = &event.details {
        details.push(d.clone());
    }

    // Add evidence count
    if !event.evidence.is_empty() {
        details.push(format!(
            "{} source record(s) available for verification",
            event.evidence.len()
        ));
    }

    details.join("\n")
}

fn generate_suggested_action(event: &Event) -> Option<String> {
    match event.event_type {
        EventType::ServiceFailed => {
            let svc = event.service.as_deref().unwrap_or("the service");
            Some(format!(
                "Check service status with: systemctl status {}",
                svc
            ))
        }
        EventType::PermissionDenied => Some(
            "Check SELinux/AppArmor audit logs: ausearch -m avc -ts recent".to_string(),
        ),
        EventType::DiskSpaceCritical | EventType::DiskSpaceWarning => {
            Some("Check disk usage with: df -h".to_string())
        }
        EventType::DiskInodeExhaustion => {
            Some("Check inode usage with: df -i".to_string())
        }
        EventType::NetworkLinkDown | EventType::NetworkDhcpFailure => {
            Some("Check network status with: nmcli general status".to_string())
        }
        EventType::KernelError => {
            Some("Check kernel logs with: journalctl -k --since '1 hour ago'".to_string())
        }
        _ => None,
    }
}

fn format_timestamp(ts: DateTime<Utc>) -> String {
    let local: DateTime<Local> = ts.into();
    local.format("%H:%M:%S").to_string()
}

/// Generates a daily summary of system activity.
#[must_use]
pub fn generate_daily_summary(events: &[Event], groups: &[CorrelationGroup]) -> String {
    let mut summary = String::new();

    // Count by severity
    let critical_count = events.iter().filter(|e| e.severity == Severity::Critical).count();
    let error_count = events.iter().filter(|e| e.severity == Severity::Error).count();
    let warning_count = events.iter().filter(|e| e.severity == Severity::Warning).count();

    summary.push_str(&format!(
        "System Activity Summary: {} events ({} critical, {} errors, {} warnings)\n\n",
        events.len(),
        critical_count,
        error_count,
        warning_count
    ));

    if !groups.is_empty() {
        summary.push_str(&format!(
            "Identified {} correlated event group(s):\n",
            groups.len()
        ));
        for group in groups {
            summary.push_str(&format!(
                "  • {} (confidence: {}%)\n",
                group.rule_title, group.confidence
            ));
        }
    }

    summary
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_summarize_event() {
        let event = Event::new(
            Utc::now(),
            EventType::ServiceFailed,
            Severity::Error,
            "nginx.service failed to start".to_string(),
        )
        .with_service("nginx.service");

        let summary = summarize_event(&event);

        assert!(summary.headline.contains("nginx.service failed"));
        assert!(summary.suggested_action.is_some());
        assert!(summary
            .suggested_action
            .unwrap()
            .contains("systemctl status"));
    }

    #[test]
    fn test_severity_icons() {
        let info_event = Event::new(
            Utc::now(),
            EventType::ServiceStart,
            Severity::Info,
            "Test".to_string(),
        );
        let critical_event = Event::new(
            Utc::now(),
            EventType::KernelError,
            Severity::Critical,
            "Test".to_string(),
        );

        let info_summary = summarize_event(&info_event);
        let critical_summary = summarize_event(&critical_event);

        assert!(info_summary.headline.contains("ℹ️"));
        assert!(critical_summary.headline.contains("🔴"));
    }
}
