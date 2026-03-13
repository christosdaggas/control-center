//! Correlation rule trait and types.
//!
//! Defines the interface that all correlation rules must implement.

use crate::domain::event::Event;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata describing a correlation rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMetadata {
    /// Unique identifier for this rule.
    pub id: String,

    /// Human-readable title.
    pub title: String,

    /// Detailed description of what this rule detects.
    pub description: String,

    /// Priority (lower = higher priority).
    pub priority: u8,

    /// Maximum time window for correlation (e.g., 5 minutes).
    pub time_window: Duration,
}

/// A match found by a correlation rule.
#[derive(Debug, Clone)]
pub struct RuleMatch {
    /// The events that matched this rule.
    pub event_ids: Vec<Uuid>,

    /// The primary/root cause event (usually the first).
    pub primary_event_id: Uuid,

    /// Confidence score (0-100).
    pub confidence: u8,

    /// Human-readable explanation of why these events match.
    pub explanation: String,
}

impl RuleMatch {
    /// Creates a new rule match.
    #[must_use]
    pub fn new(
        event_ids: Vec<Uuid>,
        primary_event_id: Uuid,
        confidence: u8,
        explanation: String,
    ) -> Self {
        Self {
            event_ids,
            primary_event_id,
            confidence: confidence.min(100),
            explanation,
        }
    }
}

/// A group of correlated events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationGroup {
    /// Unique identifier for this group.
    pub id: Uuid,

    /// The events in this group.
    pub events: Vec<Event>,

    /// ID of the rule that created this group.
    pub rule_id: String,

    /// Title of the rule.
    pub rule_title: String,

    /// Confidence score (0-100).
    pub confidence: u8,

    /// Human-readable explanation of why these events are grouped.
    pub explanation: String,

    /// ID of the primary event (likely root cause).
    pub primary_event_id: Uuid,
}

impl CorrelationGroup {
    /// Creates a new correlation group from a rule match.
    #[must_use]
    pub fn from_match(
        rule: &RuleMetadata,
        rule_match: &RuleMatch,
        events: Vec<Event>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            events,
            rule_id: rule.id.clone(),
            rule_title: rule.title.clone(),
            confidence: rule_match.confidence,
            explanation: rule_match.explanation.clone(),
            primary_event_id: rule_match.primary_event_id,
        }
    }

    /// Returns the primary event (likely root cause).
    #[must_use]
    pub fn primary_event(&self) -> Option<&Event> {
        self.events.iter().find(|e| e.id == self.primary_event_id)
    }

    /// Returns all events except the primary one.
    #[must_use]
    pub fn secondary_events(&self) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.id != self.primary_event_id)
            .collect()
    }
}

/// Trait that all correlation rules must implement.
///
/// Rules are deterministic and must provide transparent reasoning
/// for every correlation they produce.
pub trait Rule: Send + Sync {
    /// Returns metadata about this rule.
    fn metadata(&self) -> &RuleMetadata;

    /// Attempts to find correlations in the given events.
    ///
    /// Returns a list of matches (may be empty).
    /// Each match represents a group of related events.
    fn find_matches(&self, events: &[Event]) -> Vec<RuleMatch>;
}

/// Helper to calculate confidence based on time proximity.
///
/// Returns higher confidence for events closer together in time.
#[must_use]
pub fn time_proximity_confidence(
    actual_gap: Duration,
    max_window: Duration,
    base_confidence: u8,
) -> u8 {
    if actual_gap >= max_window {
        return 0;
    }

    let max_ms = max_window.num_milliseconds() as f64;
    let actual_ms = actual_gap.num_milliseconds() as f64;

    // Linear decay: closer events get higher confidence
    let ratio = 1.0 - (actual_ms / max_ms);
    let boost = (ratio * 15.0) as u8; // Up to 15 extra points

    base_confidence.saturating_add(boost).min(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_proximity_confidence() {
        let max_window = Duration::minutes(5);

        // Very close events get higher confidence
        let close = time_proximity_confidence(Duration::seconds(30), max_window, 80);
        let far = time_proximity_confidence(Duration::minutes(4), max_window, 80);

        assert!(close > far);
        assert!(close >= 80);
        assert!(far >= 80);
    }

    #[test]
    fn test_confidence_capped_at_100() {
        let max_window = Duration::minutes(5);
        let conf = time_proximity_confidence(Duration::seconds(1), max_window, 95);
        assert!(conf <= 100);
    }
}
