//! Ingest events use case.

use crate::application::services::Services;
use crate::domain::event::Event;
use crate::infrastructure::adapters::AdapterError;
use chrono::{DateTime, Utc};
use tracing::{info, instrument};

/// Ingests events from all available sources.
#[instrument(skip(services))]
pub fn ingest_events(
    services: &Services,
    since: DateTime<Utc>,
) -> Result<Vec<Event>, AdapterError> {
    info!(%since, "Ingesting events from all sources");

    let events = services.ingestion.ingest_all(since)?;

    info!(count = events.len(), "Events ingested successfully");
    Ok(events)
}
