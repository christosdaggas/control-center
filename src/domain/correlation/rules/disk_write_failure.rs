//! Disk Full → Write Failure correlation rule.
//!
//! Detects when disk space issues cause write failures in services.

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType, Severity};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "disk-write-fail".to_string(),
    title: "Disk Space → Write Failures".to_string(),
    description: "Correlates disk space warnings/critical events with subsequent \
                  service failures that may be caused by inability to write."
        .to_string(),
    priority: 3,
    time_window: Duration::minutes(10),
});

/// Rule that correlates disk space issues with service failures.
pub struct DiskWriteFailureRule;

impl DiskWriteFailureRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for DiskWriteFailureRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DiskWriteFailureRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Find disk space events
        let disk_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    EventType::DiskSpaceWarning
                        | EventType::DiskSpaceCritical
                        | EventType::DiskInodeExhaustion
                )
            })
            .collect();

        // Find service failures
        let service_failures: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::ServiceFailed && e.severity >= Severity::Error)
            .collect();

        for disk_event in &disk_events {
            let mut related_failures = Vec::new();

            for failure in &service_failures {
                let time_gap = failure.timestamp - disk_event.timestamp;

                // Failure should come after or at the same time as disk event
                if time_gap >= Duration::seconds(-30) && time_gap <= METADATA.time_window {
                    related_failures.push(*failure);
                }
            }

            if !related_failures.is_empty() {
                let mut event_ids: Vec<_> = vec![disk_event.id];
                event_ids.extend(related_failures.iter().map(|e| e.id));

                // Higher confidence for critical disk events
                let base_confidence = match disk_event.event_type {
                    EventType::DiskSpaceCritical | EventType::DiskInodeExhaustion => 90,
                    EventType::DiskSpaceWarning => 75,
                    _ => 70,
                };

                let avg_gap = related_failures
                    .iter()
                    .map(|e| (e.timestamp - disk_event.timestamp).num_seconds().max(0))
                    .sum::<i64>()
                    / related_failures.len().max(1) as i64;

                let confidence = time_proximity_confidence(
                    Duration::seconds(avg_gap),
                    METADATA.time_window,
                    base_confidence,
                );

                let disk_issue = disk_event.event_type.label();
                let failed_services: Vec<_> = related_failures
                    .iter()
                    .filter_map(|e| e.service.as_ref())
                    .map(String::as_str)
                    .collect();

                let explanation = format!(
                    "{} detected, followed by {} service failure(s) ({}). \
                     Services may be failing due to inability to write logs, \
                     data, or temporary files.",
                    disk_issue,
                    related_failures.len(),
                    if failed_services.is_empty() {
                        "unknown services".to_string()
                    } else {
                        failed_services.join(", ")
                    }
                );

                matches.push(RuleMatch::new(
                    event_ids,
                    disk_event.id,
                    confidence,
                    explanation,
                ));
            }
        }

        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_disk_write_failure_correlation() {
        let now = Utc::now();

        let events = vec![
            Event::new(
                now,
                EventType::DiskSpaceCritical,
                Severity::Critical,
                "Disk /var is 95% full".to_string(),
            ),
            Event::new(
                now + Duration::seconds(60),
                EventType::ServiceFailed,
                Severity::Error,
                "postgresql.service failed".to_string(),
            )
            .with_service("postgresql.service"),
        ];

        let rule = DiskWriteFailureRule::new();
        let matches = rule.find_matches(&events);

        assert_eq!(matches.len(), 1);
        assert!(matches[0].confidence >= 85);
        assert!(matches[0].explanation.contains("Disk Space Critical"));
    }
}
