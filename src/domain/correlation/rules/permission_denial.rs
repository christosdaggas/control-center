//! Permission Denial Impact correlation rule.
//!
//! Detects when SELinux/AppArmor denials cause service errors.

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "permission-denial".to_string(),
    title: "Permission Denial → Service Impact".to_string(),
    description: "Correlates SELinux/AppArmor permission denials with subsequent \
                  service errors that may be caused by the denial."
        .to_string(),
    priority: 4,
    time_window: Duration::minutes(5),
});

/// Rule that correlates permission denials with service errors.
pub struct PermissionDenialImpactRule;

impl PermissionDenialImpactRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PermissionDenialImpactRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for PermissionDenialImpactRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Find permission denial events
        let denials: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::PermissionDenied)
            .collect();

        // Find service errors
        let service_errors: Vec<_> = events
            .iter()
            .filter(|e| {
                e.event_type == EventType::ServiceFailed && e.service.is_some()
            })
            .collect();

        for denial in &denials {
            // Try to extract the affected service from the denial event
            let denial_service = denial.service.as_ref();

            let mut related_errors = Vec::new();

            for error in &service_errors {
                let error_service = error.service.as_ref();
                let time_gap = error.timestamp - denial.timestamp;

                // Error should come after denial
                if time_gap < Duration::zero() || time_gap > METADATA.time_window {
                    continue;
                }

                // Check if services match (if we know the denied service)
                let services_match = match (denial_service, error_service) {
                    (Some(d), Some(e)) => {
                        d == e || d.contains(e.as_str()) || e.contains(d.as_str())
                    }
                    _ => true, // If we don't know the denied service, include all errors
                };

                if services_match {
                    related_errors.push(*error);
                }
            }

            if !related_errors.is_empty() {
                let mut event_ids = vec![denial.id];
                event_ids.extend(related_errors.iter().map(|e| e.id));

                // Lower base confidence since we can't always be certain
                let base_confidence = if denial.service.is_some() { 75 } else { 60 };

                let avg_gap = related_errors
                    .iter()
                    .map(|e| (e.timestamp - denial.timestamp).num_seconds())
                    .sum::<i64>()
                    / related_errors.len() as i64;

                let confidence = time_proximity_confidence(
                    Duration::seconds(avg_gap),
                    METADATA.time_window,
                    base_confidence,
                );

                let denied_context = denial.service.as_deref().unwrap_or("unknown process");
                let failed_services: Vec<_> = related_errors
                    .iter()
                    .filter_map(|e| e.service.as_ref())
                    .collect();

                let explanation = format!(
                    "Permission denied for '{}', followed by service error(s) in {}. \
                     Check SELinux/AppArmor audit logs for details.",
                    denied_context,
                    if failed_services.is_empty() {
                        "related services".to_string()
                    } else {
                        failed_services
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                );

                matches.push(RuleMatch::new(
                    event_ids,
                    denial.id,
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
    use crate::domain::event::Severity;
    use chrono::Utc;

    #[test]
    fn test_permission_denial_correlation() {
        let now = Utc::now();

        let events = vec![
            Event::new(
                now,
                EventType::PermissionDenied,
                Severity::Warning,
                "SELinux denied nginx access to /var/log/custom".to_string(),
            )
            .with_service("nginx"),
            Event::new(
                now + Duration::seconds(5),
                EventType::ServiceFailed,
                Severity::Error,
                "nginx.service failed".to_string(),
            )
            .with_service("nginx.service"),
        ];

        let rule = PermissionDenialImpactRule::new();
        let matches = rule.find_matches(&events);

        assert_eq!(matches.len(), 1);
        assert!(matches[0].explanation.contains("nginx"));
        assert!(matches[0].explanation.contains("SELinux/AppArmor"));
    }
}
