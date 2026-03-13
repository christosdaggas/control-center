//! Control Center - Linux System Change & Activity Timeline Viewer
//!
//! This application provides a unified timeline of meaningful system changes,
//! turning logs and events into an understandable narrative.

// Enforce strict linting in addition to Cargo.toml settings
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

pub mod application;
pub mod config;
pub mod domain;
pub mod i18n;
pub mod infrastructure;
pub mod ui;
pub mod version_check;

pub use config::Config;
