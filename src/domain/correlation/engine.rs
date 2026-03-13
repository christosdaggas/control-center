//! Correlation engine that applies rules to events.
//!
//! The engine runs all registered rules against an event stream
//! and produces correlated groups with transparent reasoning.

use super::rule::{CorrelationGroup, Rule};
use crate::domain::event::Event;
use std::collections::HashSet;
use tracing::{debug, instrument, span, Level};
use uuid::Uuid;

/// The correlation engine that applies rules to events.
pub struct CorrelationEngine {
    rules: Vec<Box<dyn Rule>>,
}

impl CorrelationEngine {
    /// Creates a new correlation engine with no rules.
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Registers a rule with the engine.
    pub fn register_rule(&mut self, rule: Box<dyn Rule>) {
        debug!(rule_id = %rule.metadata().id, "Registering correlation rule");
        self.rules.push(rule);
    }

    /// Returns the number of registered rules.
    #[must_use]
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Correlates events using all registered rules.
    ///
    /// Rules are applied in priority order. Once an event is part of a group,
    /// it won't be matched by lower-priority rules (to avoid duplicate groupings).
    #[instrument(skip(self, events), fields(event_count = events.len()))]
    pub fn correlate(&self, events: &[Event]) -> Vec<CorrelationGroup> {
        let _span = span!(Level::DEBUG, "correlation").entered();

        // Sort rules by priority
        let mut sorted_rules: Vec<_> = self.rules.iter().collect();
        sorted_rules.sort_by_key(|r| r.metadata().priority);

        let mut groups: Vec<CorrelationGroup> = Vec::new();
        let mut claimed_events: HashSet<Uuid> = HashSet::new();

        for rule in sorted_rules {
            let metadata = rule.metadata();
            debug!(rule_id = %metadata.id, "Applying rule");

            // Filter to unclaimed events only
            let available_events: Vec<_> = events
                .iter()
                .filter(|e| !claimed_events.contains(&e.id))
                .cloned()
                .collect();

            if available_events.is_empty() {
                continue;
            }

            let matches = rule.find_matches(&available_events);

            for rule_match in matches {
                // Skip matches with zero confidence
                if rule_match.confidence == 0 {
                    continue;
                }

                // Collect the actual events for this match
                let matched_events: Vec<Event> = available_events
                    .iter()
                    .filter(|e| rule_match.event_ids.contains(&e.id))
                    .cloned()
                    .collect();

                if matched_events.len() < 2 {
                    // Need at least 2 events for a correlation
                    continue;
                }

                // Mark events as claimed
                for event in &matched_events {
                    claimed_events.insert(event.id);
                }

                let group = CorrelationGroup::from_match(metadata, &rule_match, matched_events);

                debug!(
                    rule_id = %metadata.id,
                    group_size = group.events.len(),
                    confidence = group.confidence,
                    "Created correlation group"
                );

                groups.push(group);
            }
        }

        // Sort groups by primary event timestamp
        groups.sort_by(|a, b| {
            let a_time = a.primary_event().map(|e| e.timestamp);
            let b_time = b.primary_event().map(|e| e.timestamp);
            a_time.cmp(&b_time)
        });

        groups
    }

    /// Returns events that were not matched by any rule.
    #[must_use]
    pub fn uncorrelated_events<'a>(
        &self,
        events: &'a [Event],
        groups: &[CorrelationGroup],
    ) -> Vec<&'a Event> {
        let correlated_ids: HashSet<Uuid> = groups
            .iter()
            .flat_map(|g| g.events.iter().map(|e| e.id))
            .collect();

        events
            .iter()
            .filter(|e| !correlated_ids.contains(&e.id))
            .collect()
    }
}

impl Default for CorrelationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a correlation engine with all built-in rules registered.
#[must_use]
pub fn create_default_engine() -> CorrelationEngine {
    use super::rules;

    let mut engine = CorrelationEngine::new();

    // Register rules in priority order
    engine.register_rule(Box::new(rules::PackageServiceRestartRule::new()));
    engine.register_rule(Box::new(rules::ServiceCascadeFailureRule::new()));
    engine.register_rule(Box::new(rules::DiskWriteFailureRule::new()));
    engine.register_rule(Box::new(rules::PermissionDenialImpactRule::new()));
    engine.register_rule(Box::new(rules::OomServiceRestartRule::new()));
    engine.register_rule(Box::new(rules::NetworkServiceTimeoutRule::new()));
    engine.register_rule(Box::new(rules::ConfigServiceReloadRule::new()));

    engine
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{EventType, Severity};
    use chrono::Utc;

    fn make_event(event_type: EventType, service: Option<&str>) -> Event {
        let mut event = Event::new(
            Utc::now(),
            event_type,
            Severity::Info,
            "Test event".to_string(),
        );
        if let Some(svc) = service {
            event.service = Some(svc.to_string());
        }
        event
    }

    #[test]
    fn test_engine_creation() {
        let engine = CorrelationEngine::new();
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn test_default_engine_has_rules() {
        let engine = create_default_engine();
        assert!(engine.rule_count() > 0);
    }

    #[test]
    fn test_uncorrelated_events() {
        let engine = CorrelationEngine::new();
        let events = vec![
            make_event(EventType::ServiceStart, Some("nginx.service")),
            make_event(EventType::ServiceStop, Some("postgres.service")),
        ];

        let groups = engine.correlate(&events);
        let uncorrelated = engine.uncorrelated_events(&events, &groups);

        // With no rules, all events should be uncorrelated
        assert_eq!(uncorrelated.len(), 2);
    }
}
