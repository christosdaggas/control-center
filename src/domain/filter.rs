//! Event filtering and noise suppression.
//!
//! This module provides logic for filtering events to show only
//! meaningful changes and suppress background noise.

use super::event::{Event, EventType, Severity};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Predefined filter presets for common use cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterPreset {
    /// Show all events without filtering.
    All,
    /// Show events since the last system boot.
    SinceLastReboot,
    /// Show only error and critical events.
    ErrorsOnly,
    /// Show only warning events.
    WarningsOnly,
    /// Show only change events (package updates, service changes).
    ChangesOnly,
    /// Show events from "before it worked" (user-defined baseline).
    BeforeItWorked,
    /// Show events from a custom time range.
    CustomRange,
}

impl FilterPreset {
    /// Returns a human-readable label for this preset.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::All => "All Events",
            Self::SinceLastReboot => "Since Reboot",
            Self::ErrorsOnly => "Errors",
            Self::WarningsOnly => "Warnings",
            Self::ChangesOnly => "Changes",
            Self::BeforeItWorked => "Before It Worked",
            Self::CustomRange => "Custom Range",
        }
    }

    /// Returns a description for this preset.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::All => "Show all system events",
            Self::SinceLastReboot => "Events since the last system boot",
            Self::ErrorsOnly => "Only error and critical events",
            Self::WarningsOnly => "Only warning events",
            Self::ChangesOnly => "Service and package changes",
            Self::BeforeItWorked => "Compare events before your selected baseline",
            Self::CustomRange => "Events from a custom time range",
        }
    }
}

/// Configuration for filtering events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// The active preset.
    pub preset: FilterPreset,

    /// Minimum severity to include.
    pub min_severity: Option<Severity>,

    /// Maximum severity to include (for exact matches like "warnings only").
    pub max_severity: Option<Severity>,

    /// Start of time range (inclusive).
    pub time_start: Option<DateTime<Utc>>,

    /// End of time range (inclusive).
    pub time_end: Option<DateTime<Utc>>,

    /// Event types to include (empty = all).
    pub include_types: Vec<EventType>,

    /// Event types to exclude.
    pub exclude_types: Vec<EventType>,

    /// Services to filter by (empty = all).
    pub services: Vec<String>,

    /// Packages to filter by (empty = all).
    pub packages: Vec<String>,

    /// Free-text search query.
    pub search_query: Option<String>,

    /// Whether to hide repetitive/periodic events.
    pub suppress_noise: bool,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            preset: FilterPreset::SinceLastReboot,
            min_severity: None,
            max_severity: None,
            time_start: None,
            time_end: None,
            include_types: Vec::new(),
            exclude_types: Vec::new(),
            services: Vec::new(),
            packages: Vec::new(),
            search_query: None,
            suppress_noise: true,
        }
    }
}

impl FilterConfig {
    /// Creates a filter for the "Since Last Reboot" preset.
    #[must_use]
    pub fn since_last_reboot(boot_time: DateTime<Utc>) -> Self {
        Self {
            preset: FilterPreset::SinceLastReboot,
            time_start: Some(boot_time),
            suppress_noise: true,
            ..Self::default()
        }
    }

    /// Creates a filter for errors only.
    #[must_use]
    pub fn errors_only() -> Self {
        Self {
            preset: FilterPreset::ErrorsOnly,
            min_severity: Some(Severity::Error),
            suppress_noise: true,
            ..Self::default()
        }
    }

    /// Creates a filter for warnings only.
    #[must_use]
    pub fn warnings_only() -> Self {
        Self {
            preset: FilterPreset::WarningsOnly,
            min_severity: Some(Severity::Warning),
            max_severity: Some(Severity::Warning),
            suppress_noise: true,
            ..Self::default()
        }
    }

    /// Creates a filter for change-focused events.
    #[must_use]
    pub fn changes_only() -> Self {
        Self {
            preset: FilterPreset::ChangesOnly,
            include_types: vec![
                EventType::PackageInstall,
                EventType::PackageUpdate,
                EventType::PackageRemove,
                EventType::ServiceRestart,
                EventType::SystemBoot,
            ],
            suppress_noise: true,
            ..Self::default()
        }
    }
}

/// Filters a list of events according to the given configuration.
///
/// Returns a new vector containing only the events that pass the filter.
#[must_use]
pub fn filter_events(events: &[Event], config: &FilterConfig) -> Vec<Event> {
    tracing::debug!(
        total = events.len(),
        min_severity = ?config.min_severity,
        include_types_count = config.include_types.len(),
        time_start = ?config.time_start,
        "Filtering events"
    );
    
    let result: Vec<Event> = events
        .iter()
        .filter(|event| matches_filter(event, config))
        .cloned()
        .collect();
    
    tracing::debug!(filtered = result.len(), "Filter complete");
    result
}

/// Returns true if an event passes the filter configuration.
#[must_use]
pub fn matches_filter(event: &Event, config: &FilterConfig) -> bool {
    // Check minimum severity
    if let Some(min_severity) = config.min_severity {
        if event.severity < min_severity {
            tracing::trace!(event_severity = ?event.severity, min = ?min_severity, "Filtering out by severity");
            return false;
        }
    }

    // Check maximum severity (for exact filtering like "warnings only")
    if let Some(max_severity) = config.max_severity {
        if event.severity > max_severity {
            return false;
        }
    }

    // Check time range
    if let Some(start) = config.time_start {
        if event.timestamp < start {
            return false;
        }
    }
    if let Some(end) = config.time_end {
        if event.timestamp > end {
            return false;
        }
    }

    // Check included types (if specified)
    if !config.include_types.is_empty() && !config.include_types.contains(&event.event_type) {
        return false;
    }

    // Check excluded types
    if config.exclude_types.contains(&event.event_type) {
        return false;
    }

    // Check service filter
    if !config.services.is_empty() {
        match &event.service {
            Some(svc) if config.services.contains(svc) => {}
            _ => return false,
        }
    }

    // Check package filter
    if !config.packages.is_empty() {
        match &event.package {
            Some(pkg) if config.packages.contains(pkg) => {}
            _ => return false,
        }
    }

    // Check search query
    if let Some(query) = &config.search_query {
        let query_lower = query.to_lowercase();
        let matches = event.summary.to_lowercase().contains(&query_lower)
            || event
                .details
                .as_ref()
                .map_or(false, |d| d.to_lowercase().contains(&query_lower))
            || event
                .service
                .as_ref()
                .map_or(false, |s| s.to_lowercase().contains(&query_lower))
            || event
                .package
                .as_ref()
                .map_or(false, |p| p.to_lowercase().contains(&query_lower));
        if !matches {
            return false;
        }
    }

    true
}

/// Identifies and marks events that are likely periodic/background noise.
///
/// Returns indices of events that should be suppressed.
#[must_use]
pub fn identify_noise(events: &[Event]) -> Vec<usize> {
    let mut noise_indices = Vec::new();

    // Identify repetitive patterns (same event type + service within short intervals)
    for (i, event) in events.iter().enumerate() {
        if is_likely_noise(event, events, i) {
            noise_indices.push(i);
        }
    }

    noise_indices
}

/// Heuristic to determine if an event is likely background noise.
fn is_likely_noise(event: &Event, all_events: &[Event], current_idx: usize) -> bool {
    // Count similar events in a 5-minute window
    let window = Duration::minutes(5);
    let similar_count = all_events
        .iter()
        .enumerate()
        .filter(|(i, other)| {
            *i != current_idx
                && other.event_type == event.event_type
                && other.service == event.service
                && (other.timestamp - event.timestamp).abs() < window
        })
        .count();

    // If there are many similar events in a short window, it's likely noise
    // (e.g., periodic health checks, timer units)
    similar_count >= 5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_type: EventType, severity: Severity) -> Event {
        Event::new(Utc::now(), event_type, severity, "Test event".to_string())
    }

    #[test]
    fn test_filter_by_severity() {
        let events = vec![
            make_event(EventType::ServiceStart, Severity::Info),
            make_event(EventType::ServiceFailed, Severity::Error),
            make_event(EventType::KernelError, Severity::Critical),
        ];

        let config = FilterConfig {
            min_severity: Some(Severity::Warning),
            ..FilterConfig::default()
        };
        let filtered = filter_events(&events, &config);

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.severity >= Severity::Warning));
    }

    #[test]
    fn test_filter_by_type() {
        let events = vec![
            make_event(EventType::PackageInstall, Severity::Info),
            make_event(EventType::ServiceStart, Severity::Info),
            make_event(EventType::PackageUpdate, Severity::Info),
        ];

        let config = FilterConfig::changes_only();
        let filtered = filter_events(&events, &config);

        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|e| matches!(e.event_type, EventType::PackageInstall | EventType::PackageUpdate)));
    }

    #[test]
    fn test_search_query() {
        let events = vec![
            Event::new(
                Utc::now(),
                EventType::ServiceFailed,
                Severity::Error,
                "nginx.service failed to start".to_string(),
            ),
            Event::new(
                Utc::now(),
                EventType::ServiceFailed,
                Severity::Error,
                "postgresql.service failed".to_string(),
            ),
        ];

        let mut config = FilterConfig::default();
        config.search_query = Some("nginx".to_string());

        let filtered = filter_events(&events, &config);
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].summary.contains("nginx"));
    }
}
