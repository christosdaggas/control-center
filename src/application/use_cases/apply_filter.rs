//! Apply filter use case.

use crate::domain::event::Event;
use crate::domain::filter::{filter_events, FilterConfig};
use tracing::{debug, instrument};

/// Applies a filter configuration to events.
#[instrument(skip(events, config), fields(event_count = events.len()))]
pub fn apply_filter(events: &[Event], config: &FilterConfig) -> Vec<Event> {
    debug!(?config.preset, "Applying filter");

    let filtered = filter_events(events, config);

    debug!(
        original = events.len(),
        filtered = filtered.len(),
        "Filter applied"
    );

    filtered
}
