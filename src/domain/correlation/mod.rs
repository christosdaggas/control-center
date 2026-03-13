//! Correlation engine for grouping related events.
//!
//! This module contains the rule engine and correlation logic.

pub mod engine;
pub mod rule;
pub mod rules;

pub use engine::CorrelationEngine;
pub use rule::{CorrelationGroup, Rule, RuleMatch, RuleMetadata};
