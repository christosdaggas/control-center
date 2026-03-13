//! UI layer - GTK4/libadwaita widgets and pages.
//!
//! This module contains:
//! - Application setup
//! - Main window
//! - Timeline widgets
//! - Filter bar
//! - Detail pane
//! - Diagnostics page

pub mod app;
pub mod pages;
pub mod style;
pub mod widgets;
pub mod window;

pub use app::ControlCenterApp;
pub use window::MainWindow;
