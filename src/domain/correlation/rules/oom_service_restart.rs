//! OOM Kill → Service Restart correlation rule.
//!
//! Detects when an OOM kill event is followed by a service restart,
//! indicating memory pressure caused a service to be killed and restarted.

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "oom-svc-restart".to_string(),
    title: "OOM Kill → Service Restart".to_string(),
    description: "Correlates kernel OOM kill events with subsequent service restarts, \
                  indicating memory pressure caused service termination."
        .to_string(),
    priority: 2,
    time_window: Duration::minutes(3),
});

/// Rule that correlates OOM kills with service restarts.
pub struct OomServiceRestartRule;

impl OomServiceRestartRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for OomServiceRestartRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for OomServiceRestartRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Find OOM kill events (kernel errors mentioning OOM or "Out of memory")
        let oom_events: Vec<_> = events
            .iter()
            .filter(|e| {
                e.event_type == EventType::KernelError
                    && (e.summary.to_lowercase().contains("oom")
                        || e.summary.to_lowercase().contains("out of memory")
                        || e.summary.to_lowercase().contains("killed process"))
            })
            .collect();

        // Find service restart/failed events
        let service_events: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    EventType::ServiceRestart | EventType::ServiceFailed
                )
            })
            .collect();

        for oom in &oom_events {
            for svc in &service_events {
                let time_gap = svc.timestamp - oom.timestamp;
                if time_gap < Duration::zero() || time_gap > METADATA.time_window {
                    continue;
                }

                let confidence = time_proximity_confidence(time_gap, METADATA.time_window, 80);
                let svc_name = svc.service.as_deref().unwrap_or("unknown");

                let explanation = format!(
                    "OOM killer invoked, followed by {} of '{}' {} later. \
                     Memory pressure likely caused the service to be terminated.",
                    svc.event_type.label().to_lowercase(),
                    svc_name,
                    format_duration(time_gap)
                );

                matches.push(RuleMatch::new(
                    vec![oom.id, svc.id],
                    oom.id,
                    confidence,
                    explanation,
                ));
            }
        }

        matches
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.num_seconds();
    if secs < 60 {
        format!("{secs} seconds")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Severity;
    use chrono::Utc;

    #[test]
    fn test_oom_service_restart() {
        let now = Utc::now();

        let oom = Event::new(
            now,
            EventType::KernelError,
            Severity::Critical,
            "Out of memory: Killed process 1234 (nginx)".to_string(),
        );

        let restart = Event::new(
            now + Duration::seconds(10),
            EventType::ServiceRestart,
            Severity::Info,
            "nginx.service restarted".to_string(),
        )
        .with_service("nginx.service");

        let rule = OomServiceRestartRule::new();
        let results = rule.find_matches(&[oom, restart]);
        assert_eq!(results.len(), 1);
        assert!(results[0].confidence >= 80);
    }
}
