//! Application state management.
//!
//! Defines the central state model for the application.

use crate::config::Config;
use crate::domain::correlation::CorrelationGroup;
use crate::domain::event::Event;
use crate::domain::filter::FilterConfig;
use crate::infrastructure::desktop::detector::DesktopInfo;
use chrono::{DateTime, Utc};
use std::sync::{Arc, RwLock};

/// The main application state.
#[derive(Debug)]
pub struct AppState {
    /// Application configuration.
    pub config: Config,

    /// All loaded events (unfiltered).
    pub all_events: Vec<Event>,

    /// Currently displayed events (after filtering).
    pub filtered_events: Vec<Event>,

    /// Correlated event groups.
    pub correlation_groups: Vec<CorrelationGroup>,

    /// Current filter configuration.
    pub filter_config: FilterConfig,

    /// Last time events were ingested.
    pub last_ingestion: Option<DateTime<Utc>>,

    /// Total event count by source.
    pub event_counts: EventCounts,

    /// Detected desktop information.
    pub desktop_info: Option<DesktopInfo>,

    /// Loading state.
    pub is_loading: bool,

    /// Error message if any.
    pub error: Option<String>,
}

/// Event counts by source.
#[derive(Debug, Default, Clone)]
pub struct EventCounts {
    /// Events from journald.
    pub journald: usize,
    /// Events from DNF.
    pub dnf: usize,
    /// Events from APT.
    pub apt: usize,
    /// Events from kernel.
    pub kernel: usize,
    /// Total events.
    pub total: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Creates a new application state with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: Config::load().unwrap_or_default(),
            all_events: Vec::new(),
            filtered_events: Vec::new(),
            correlation_groups: Vec::new(),
            filter_config: FilterConfig::default(),
            last_ingestion: None,
            event_counts: EventCounts::default(),
            desktop_info: None,
            is_loading: false,
            error: None,
        }
    }

    /// Updates filtered events based on the current filter config.
    pub fn apply_filter(&mut self) {
        self.filtered_events = crate::domain::filter::filter_events(
            &self.all_events,
            &self.filter_config,
        );
    }

    /// Sets all events and updates filtered view.
    pub fn set_events(&mut self, events: Vec<Event>) {
        self.all_events = events;
        self.apply_filter();
        self.last_ingestion = Some(Utc::now());
        self.update_counts();
    }

    /// Updates event counts.
    fn update_counts(&mut self) {
        self.event_counts = EventCounts {
            journald: self.all_events.iter().filter(|e| {
                e.evidence.iter().any(|ev| {
                    matches!(ev.source, crate::domain::event::EvidenceSource::Journald)
                })
            }).count(),
            dnf: self.all_events.iter().filter(|e| {
                e.evidence.iter().any(|ev| {
                    matches!(
                        ev.source,
                        crate::domain::event::EvidenceSource::PackageManager(
                            crate::domain::event::PackageManagerType::Dnf
                        )
                    )
                })
            }).count(),
            apt: self.all_events.iter().filter(|e| {
                e.evidence.iter().any(|ev| {
                    matches!(
                        ev.source,
                        crate::domain::event::EvidenceSource::PackageManager(
                            crate::domain::event::PackageManagerType::Apt
                        )
                    )
                })
            }).count(),
            kernel: self.all_events.iter().filter(|e| {
                e.evidence.iter().any(|ev| {
                    matches!(ev.source, crate::domain::event::EvidenceSource::Kernel)
                })
            }).count(),
            total: self.all_events.len(),
        };
    }
}

/// Thread-safe shared state wrapper.
pub type SharedState = Arc<RwLock<AppState>>;

/// Creates a new shared state instance.
#[must_use]
pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::event::{EventType, Severity};

    #[test]
    fn test_state_creation() {
        let state = AppState::new();
        assert!(state.all_events.is_empty());
        assert!(!state.is_loading);
    }

    #[test]
    fn test_set_events() {
        let mut state = AppState::new();
        let events = vec![
            Event::new(Utc::now(), EventType::ServiceStart, Severity::Info, "Test".to_string()),
        ];

        state.set_events(events);

        assert_eq!(state.all_events.len(), 1);
        assert!(state.last_ingestion.is_some());
    }
}
