//! Service Cascade Failure correlation rule.
//!
//! Detects when a service failure causes dependent services to fail.

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType, Severity};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "svc-cascade-fail".to_string(),
    title: "Service Failure Cascade".to_string(),
    description: "Detects cascading service failures where one service failing causes \
                  other dependent services to fail."
        .to_string(),
    priority: 2,
    time_window: Duration::minutes(2),
});

/// Rule that correlates cascading service failures.
pub struct ServiceCascadeFailureRule;

impl ServiceCascadeFailureRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ServiceCascadeFailureRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ServiceCascadeFailureRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Find all service failure events (Error or Critical severity)
        let failures: Vec<_> = events
            .iter()
            .filter(|e| {
                e.event_type == EventType::ServiceFailed
                    && e.service.is_some()
                    && e.severity >= Severity::Error
            })
            .collect();

        if failures.len() < 2 {
            return matches;
        }

        // Group failures by time proximity
        // The first failure in a time window is likely the root cause
        let mut used_events = std::collections::HashSet::new();

        for (i, first_failure) in failures.iter().enumerate() {
            if used_events.contains(&first_failure.id) {
                continue;
            }

            let mut group_events = vec![(*first_failure).clone()];
            used_events.insert(first_failure.id);

            // Find subsequent failures within the time window
            for subsequent in failures.iter().skip(i + 1) {
                if used_events.contains(&subsequent.id) {
                    continue;
                }

                let time_gap = subsequent.timestamp - first_failure.timestamp;
                if time_gap >= Duration::zero() && time_gap <= METADATA.time_window {
                    group_events.push((*subsequent).clone());
                    used_events.insert(subsequent.id);
                }
            }

            // Only create a match if we have multiple failures
            if group_events.len() >= 2 {
                let event_ids: Vec<_> = group_events.iter().map(|e| e.id).collect();
                let primary_id = first_failure.id;

                // Calculate confidence based on time proximity of failures
                let avg_gap = group_events
                    .iter()
                    .skip(1)
                    .map(|e| (e.timestamp - first_failure.timestamp).num_seconds())
                    .sum::<i64>()
                    / (group_events.len() - 1) as i64;

                let confidence = time_proximity_confidence(
                    Duration::seconds(avg_gap),
                    METADATA.time_window,
                    70,
                );

                let first_svc = first_failure.service.as_ref().expect("filtered above");
                let other_svcs: Vec<String> = group_events
                    .iter()
                    .skip(1)
                    .filter_map(|e| e.service.clone())
                    .collect();

                let explanation = format!(
                    "Service '{}' failed first, followed by {} other service failure(s) ({}) \
                     within {} minutes. This suggests a cascading failure or shared dependency.",
                    first_svc,
                    other_svcs.len(),
                    other_svcs.join(", "),
                    METADATA.time_window.num_minutes()
                );

                matches.push(RuleMatch::new(event_ids, primary_id, confidence, explanation));
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
    fn test_cascade_detection() {
        let now = Utc::now();

        let events = vec![
            Event::new(
                now,
                EventType::ServiceFailed,
                Severity::Error,
                "database.service failed".to_string(),
            )
            .with_service("database.service"),
            Event::new(
                now + Duration::seconds(10),
                EventType::ServiceFailed,
                Severity::Error,
                "webapp.service failed".to_string(),
            )
            .with_service("webapp.service"),
            Event::new(
                now + Duration::seconds(15),
                EventType::ServiceFailed,
                Severity::Error,
                "cache.service failed".to_string(),
            )
            .with_service("cache.service"),
        ];

        let rule = ServiceCascadeFailureRule::new();
        let matches = rule.find_matches(&events);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].event_ids.len(), 3);
        assert!(matches[0].explanation.contains("database.service"));
    }

    #[test]
    fn test_no_cascade_if_too_far_apart() {
        let now = Utc::now();

        let events = vec![
            Event::new(
                now,
                EventType::ServiceFailed,
                Severity::Error,
                "database.service failed".to_string(),
            )
            .with_service("database.service"),
            Event::new(
                now + Duration::minutes(10), // Too far apart
                EventType::ServiceFailed,
                Severity::Error,
                "webapp.service failed".to_string(),
            )
            .with_service("webapp.service"),
        ];

        let rule = ServiceCascadeFailureRule::new();
        let matches = rule.find_matches(&events);

        assert!(matches.is_empty());
    }
}
