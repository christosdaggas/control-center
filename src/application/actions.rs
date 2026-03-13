//! Application actions for unidirectional data flow.
//!
//! Defines all actions that can modify application state.

use crate::domain::filter::FilterConfig;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Actions that can be dispatched to modify application state.
#[derive(Debug, Clone)]
pub enum AppAction {
    // Event loading
    /// Load events from all sources since the given time.
    LoadEvents {
        /// The timestamp to load events from.
        since: DateTime<Utc>,
    },
    /// Reload events using current time range.
    ReloadEvents,
    /// Loading started.
    LoadingStarted,
    /// Loading completed with events.
    LoadingCompleted {
        /// Number of events loaded.
        event_count: usize,
    },
    /// Loading failed.
    LoadingFailed {
        /// Error message describing the failure.
        error: String,
    },

    // Filtering
    /// Apply a new filter configuration.
    ApplyFilter(FilterConfig),
    /// Clear all filters (show all events).
    ClearFilters,
    /// Search for text in events.
    Search {
        /// The search query string.
        query: String,
    },

    // Selection
    /// Select an event for detail view.
    SelectEvent {
        /// The ID of the event to select.
        event_id: Uuid,
    },
    /// Clear event selection.
    ClearSelection,
    /// Expand a correlation group.
    ExpandGroup {
        /// The ID of the group to expand.
        group_id: Uuid,
    },
    /// Collapse a correlation group.
    CollapseGroup {
        /// The ID of the group to collapse.
        group_id: Uuid,
    },

    // UI state
    /// Show the diagnostics page.
    ShowDiagnostics,
    /// Hide the diagnostics page.
    HideDiagnostics,
    /// Toggle theme (system/light/dark).
    ToggleTheme,

    // Configuration
    /// Save configuration.
    SaveConfig,
}

impl AppAction {
    /// Returns a human-readable description of this action.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::LoadEvents { .. } => "Load events",
            Self::ReloadEvents => "Reload events",
            Self::LoadingStarted => "Loading started",
            Self::LoadingCompleted { .. } => "Loading completed",
            Self::LoadingFailed { .. } => "Loading failed",
            Self::ApplyFilter(_) => "Apply filter",
            Self::ClearFilters => "Clear filters",
            Self::Search { .. } => "Search",
            Self::SelectEvent { .. } => "Select event",
            Self::ClearSelection => "Clear selection",
            Self::ExpandGroup { .. } => "Expand group",
            Self::CollapseGroup { .. } => "Collapse group",
            Self::ShowDiagnostics => "Show diagnostics",
            Self::HideDiagnostics => "Hide diagnostics",
            Self::ToggleTheme => "Toggle theme",
            Self::SaveConfig => "Save configuration",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_descriptions() {
        let action = AppAction::LoadEvents { since: Utc::now() };
        assert_eq!(action.description(), "Load events");
    }
}
