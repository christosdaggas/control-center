//! Desktop environment detection and integration.

pub mod detector;
pub mod notifications;
pub mod portal;
pub mod theme;

pub use detector::{detect_desktop, DesktopEnvironment, SessionType};
pub use notifications::{send_notification, DesktopNotification, NotificationPriority};
