//! Service registry and dependency injection.
//!
//! Provides trait-based service access for testability.

use crate::domain::correlation::CorrelationEngine;
use crate::infrastructure::adapters::AdapterRegistry;
use std::sync::Arc;

/// Trait for event ingestion service.
pub trait EventIngestionService: Send + Sync {
    /// Ingest events from all available sources.
    fn ingest_all(&self, since: chrono::DateTime<chrono::Utc>) 
        -> Result<Vec<crate::domain::event::Event>, crate::infrastructure::adapters::AdapterError>;
}

/// Trait for correlation service.
pub trait CorrelationService: Send + Sync {
    /// Correlate events using registered rules.
    fn correlate(&self, events: &[crate::domain::event::Event]) 
        -> Vec<crate::domain::correlation::CorrelationGroup>;
}

/// Service container holding all application services.
pub struct Services {
    /// Event ingestion service.
    pub ingestion: Arc<dyn EventIngestionService>,
    
    /// Correlation service.
    pub correlation: Arc<dyn CorrelationService>,
}

/// Default implementation of ingestion service.
pub struct DefaultIngestionService {
    registry: AdapterRegistry,
}

impl DefaultIngestionService {
    /// Creates a new ingestion service with the given registry.
    #[must_use]
    pub fn new(registry: AdapterRegistry) -> Self {
        Self { registry }
    }
}

impl EventIngestionService for DefaultIngestionService {
    fn ingest_all(&self, since: chrono::DateTime<chrono::Utc>) 
        -> Result<Vec<crate::domain::event::Event>, crate::infrastructure::adapters::AdapterError>
    {
        self.registry.read_all_since(since)
    }
}

/// Default implementation of correlation service.
pub struct DefaultCorrelationService {
    engine: CorrelationEngine,
}

impl DefaultCorrelationService {
    /// Creates a new correlation service with the given engine.
    #[must_use]
    pub fn new(engine: CorrelationEngine) -> Self {
        Self { engine }
    }
}

impl CorrelationService for DefaultCorrelationService {
    fn correlate(&self, events: &[crate::domain::event::Event]) 
        -> Vec<crate::domain::correlation::CorrelationGroup>
    {
        self.engine.correlate(events)
    }
}

/// Creates the default service container.
#[must_use]
pub fn create_services() -> Services {
    use crate::domain::correlation::engine::create_default_engine;
    use crate::infrastructure::adapters::create_default_registry;

    Services {
        ingestion: Arc::new(DefaultIngestionService::new(create_default_registry())),
        correlation: Arc::new(DefaultCorrelationService::new(create_default_engine())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_services() {
        let services = create_services();
        // Just verify it creates without panic
        assert!(Arc::strong_count(&services.ingestion) == 1);
    }
}
