//! Config Change → Service Reload correlation rule.
//!
//! Detects when a configuration file change is followed by a service
//! restart or reload, suggesting the restart was triggered by the config change.

use crate::domain::correlation::rule::{time_proximity_confidence, Rule, RuleMatch, RuleMetadata};
use crate::domain::event::{Event, EventType};
use chrono::Duration;
use once_cell::sync::Lazy;

static METADATA: Lazy<RuleMetadata> = Lazy::new(|| RuleMetadata {
    id: "config-svc-reload".to_string(),
    title: "Config Change → Service Reload".to_string(),
    description: "Correlates configuration file changes with subsequent service \
                  restarts or reloads."
        .to_string(),
    priority: 4,
    time_window: Duration::minutes(5),
});

/// Rule that correlates configuration changes with service restarts.
pub struct ConfigServiceReloadRule;

impl ConfigServiceReloadRule {
    /// Creates a new instance of this rule.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ConfigServiceReloadRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ConfigServiceReloadRule {
    fn metadata(&self) -> &RuleMetadata {
        &METADATA
    }

    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch> {
        let mut matches = Vec::new();

        // Find events that look like config changes
        // These could be filesystem events or package updates to config packages
        let config_events: Vec<_> = events
            .iter()
            .filter(|e| {
                let msg = e.summary.to_lowercase();
                msg.contains("config")
                    || msg.contains("configuration")
                    || msg.contains("/etc/")
                    || e.evidence.iter().any(|ev| {
                        ev.file_path
                            .as_ref()
                            .map_or(false, |p| p.starts_with("/etc"))
                    })
            })
            .collect();

        // Find service restarts
        let restarts: Vec<_> = events
            .iter()
            .filter(|e| {
                matches!(
                    e.event_type,
                    EventType::ServiceRestart | EventType::ServiceStart
                ) && e.service.is_some()
            })
            .collect();

        for config_ev in &config_events {
            for restart in &restarts {
                let time_gap = restart.timestamp - config_ev.timestamp;
                if time_gap < Duration::zero() || time_gap > METADATA.time_window {
                    continue;
                }

                // Check if the config change and service are likely related
                let svc_name = restart.service.as_deref().unwrap_or("");
                let svc_base = svc_name.strip_suffix(".service").unwrap_or(svc_name);

                let related = config_ev.summary.to_lowercase().contains(svc_base)
                    || config_ev.service.as_deref() == Some(svc_name)
                    || config_ev.evidence.iter().any(|ev| {
                        ev.file_path
                            .as_ref()
                            .map_or(false, |p| {
                                p.to_string_lossy().contains(svc_base)
                            })
                    });

                if !related {
                    continue;
                }

                let confidence =
                    time_proximity_confidence(time_gap, METADATA.time_window, 70);

                let explanation = format!(
                    "Configuration change detected for '{}', followed by {} of '{}'. \
                     The restart was likely triggered by the configuration update.",
                    svc_base,
                    restart.event_type.label().to_lowercase(),
                    svc_name
                );

                matches.push(RuleMatch::new(
                    vec![config_ev.id, restart.id],
                    config_ev.id,
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
    use crate::domain::event::{Evidence, EvidenceSource, Severity};
    use chrono::Utc;
    use std::path::PathBuf;

    #[test]
    fn test_config_change_service_restart() {
        let now = Utc::now();

        let config_change = Event::new(
            now,
            EventType::Other,
            Severity::Info,
            "Modified /etc/nginx/nginx.conf".to_string(),
        )
        .with_evidence(Evidence {
            source: EvidenceSource::Filesystem,
            cursor: None,
            file_path: Some(PathBuf::from("/etc/nginx/nginx.conf")),
            raw_content: "config changed".to_string(),
            line_number: None,
        });

        let restart = Event::new(
            now + Duration::seconds(15),
            EventType::ServiceRestart,
            Severity::Info,
            "nginx.service restarted".to_string(),
        )
        .with_service("nginx.service");

        let rule = ConfigServiceReloadRule::new();
        let results = rule.find_matches(&[config_change, restart]);
        assert_eq!(results.len(), 1);
    }
}
