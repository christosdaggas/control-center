//! Application layer - Orchestration and state management.
//!
//! This module contains:
//! - Application state
//! - Action dispatch
//! - Use cases (business logic orchestration)

pub mod actions;
pub mod services;
pub mod state;
pub mod use_cases;

pub use actions::AppAction;
pub use state::AppState;
