//! Correlate events use case.

use crate::application::services::Services;
use crate::domain::correlation::CorrelationGroup;
use crate::domain::event::Event;
use tracing::{debug, instrument};

/// Correlates events using the registered correlation rules.
#[instrument(skip(services, events), fields(event_count = events.len()))]
pub fn correlate_events(services: &Services, events: &[Event]) -> Vec<CorrelationGroup> {
    debug!("Correlating events");

    let groups = services.correlation.correlate(events);

    debug!(group_count = groups.len(), "Correlation complete");

    groups
}
