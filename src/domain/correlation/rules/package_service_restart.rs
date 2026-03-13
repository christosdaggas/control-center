//! Package Update → Service Restart correlation rule.
//!
//! Detects when a package update is followed by a service restart
//! for a service provided by that package.

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "pkg-svc-restart".to_string(),
    title: "Package Update → Service Restart".to_string(),
    description: "Correlates package updates with subsequent service restarts for services \
                  provided by those packages."
        .to_string(),
    priority: 1,
    time_window: Duration::minutes(5),
});

/// Rule that correlates package updates with service restarts.
pub struct PackageServiceRestartRule;

impl PackageServiceRestartRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PackageServiceRestartRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for PackageServiceRestartRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Find all package update events
        let package_updates: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::PackageUpdate && e.package.is_some())
            .collect();

        // Find all service restart events
        let service_restarts: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == EventType::ServiceRestart && e.service.is_some())
            .collect();

        for pkg_event in &package_updates {
            let pkg_name = pkg_event.package.as_ref().expect("filtered above");

            for svc_event in &service_restarts {
                let svc_name = svc_event.service.as_ref().expect("filtered above");

                // Check if the service name contains the package name
                // (e.g., nginx.service for nginx package)
                let service_matches = svc_name.starts_with(pkg_name)
                    || svc_name.contains(&format!("-{pkg_name}"))
                    || svc_name.contains(&format!("{pkg_name}-"));

                if !service_matches {
                    continue;
                }

                // Check time window: restart should be after update
                let time_gap = svc_event.timestamp - pkg_event.timestamp;
                if time_gap < Duration::zero() || time_gap > METADATA.time_window {
                    continue;
                }

                let confidence = time_proximity_confidence(time_gap, METADATA.time_window, 85);

                let explanation = format!(
                    "Package '{}' was updated, followed by restart of '{}' {} later. \
                     This is expected behavior after package updates.",
                    pkg_name,
                    svc_name,
                    format_duration(time_gap)
                );

                matches.push(RuleMatch::new(
                    vec![pkg_event.id, svc_event.id],
                    pkg_event.id,
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
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        if remaining_secs == 0 {
            format!("{mins} minute(s)")
        } else {
            format!("{mins}m {remaining_secs}s")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::Severity;
    use chrono::{Duration, Utc};

    #[test]
    fn test_package_service_restart_correlation() {
        let now = Utc::now();

        let pkg_event = Event::new(
            now,
            EventType::PackageUpdate,
            Severity::Info,
            "Updated nginx to 1.24.0".to_string(),
        )
        .with_package("nginx");

        let svc_event = Event::new(
            now + Duration::seconds(30),
            EventType::ServiceRestart,
            Severity::Info,
            "nginx.service restarted".to_string(),
        )
        .with_service("nginx.service");

        let events = vec![pkg_event, svc_event];
        let rule = PackageServiceRestartRule::new();
        let matches = rule.find_matches(&events);

        assert_eq!(matches.len(), 1);
        assert!(matches[0].confidence >= 85);
        assert!(matches[0].explanation.contains("nginx"));
    }

    #[test]
    fn test_no_match_if_unrelated() {
        let now = Utc::now();

        let pkg_event = Event::new(
            now,
            EventType::PackageUpdate,
            Severity::Info,
            "Updated nginx to 1.24.0".to_string(),
        )
        .with_package("nginx");

        let svc_event = Event::new(
            now + Duration::seconds(30),
            EventType::ServiceRestart,
            Severity::Info,
            "postgresql.service restarted".to_string(),
        )
        .with_service("postgresql.service");

        let events = vec![pkg_event, svc_event];
        let rule = PackageServiceRestartRule::new();
        let matches = rule.find_matches(&events);

        assert!(matches.is_empty());
    }
}
