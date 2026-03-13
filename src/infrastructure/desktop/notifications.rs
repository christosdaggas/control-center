//! Desktop notification support via GIO/GNotification.
//!
//! Sends desktop notifications for critical system events like service
//! failures, disk space critical, and OOM kills. Integrates with the
//! user's notification system via GTK's `GNotification` API.

use gtk4::gio;
use gtk4::prelude::*;
use tracing::{debug, warn};

/// Priority levels for notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPriority {
    /// Low priority, may be silently queued.
    Low,
    /// Normal priority.
    Normal,
    /// High priority, shown prominently.
    High,
    /// Urgent priority, may bypass DND.
    Urgent,
}

impl NotificationPriority {
    /// Converts to GIO priority.
    fn to_gio(self) -> gio::NotificationPriority {
        match self {
            Self::Low => gio::NotificationPriority::Low,
            Self::Normal => gio::NotificationPriority::Normal,
            Self::High => gio::NotificationPriority::High,
            Self::Urgent => gio::NotificationPriority::Urgent,
        }
    }
}

/// A notification to send to the desktop.
#[derive(Debug, Clone)]
pub struct DesktopNotification {
    /// Unique ID for the notification (allows replacing/withdrawing).
    pub id: String,
    /// Notification title.
    pub title: String,
    /// Notification body text.
    pub body: String,
    /// Priority level.
    pub priority: NotificationPriority,
    /// Icon name (freedesktop icon name).
    pub icon: Option<String>,
}

impl DesktopNotification {
    /// Creates a new notification.
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
            priority: NotificationPriority::Normal,
            icon: None,
        }
    }

    /// Sets the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the icon name.
    #[must_use]
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Sends a desktop notification via the GIO Application.
///
/// The application must be registered and running for this to work.
pub fn send_notification(notification: &DesktopNotification) {
    let app = match gio::Application::default() {
        Some(app) => app,
        None => {
            warn!("No GIO Application available for sending notifications");
            return;
        }
    };

    let notif = gio::Notification::new(&notification.title);
    notif.set_body(Some(&notification.body));
    notif.set_priority(notification.priority.to_gio());

    if let Some(ref icon_name) = notification.icon {
        let icon = gio::ThemedIcon::new(icon_name);
        notif.set_icon(&icon);
    }

    app.send_notification(Some(&notification.id), &notif);
    debug!(
        id = %notification.id,
        title = %notification.title,
        "Desktop notification sent"
    );
}

/// Withdraws a previously sent notification.
pub fn withdraw_notification(id: &str) {
    if let Some(app) = gio::Application::default() {
        app.withdraw_notification(id);
        debug!(id = %id, "Notification withdrawn");
    }
}

/// Creates and sends a notification for a service failure.
pub fn notify_service_failure(service_name: &str) {
    let notification = DesktopNotification::new(
        format!("service-failure-{}", service_name),
        "Service Failed",
        format!("The service '{}' has failed. Check the Services page for details.", service_name),
    )
    .with_priority(NotificationPriority::High)
    .with_icon("dialog-error-symbolic");

    send_notification(&notification);
}

/// Creates and sends a notification for critical disk space.
pub fn notify_disk_critical(mount_point: &str, usage_percent: f64) {
    let notification = DesktopNotification::new(
        format!("disk-critical-{}", mount_point.replace('/', "_")),
        "Disk Space Critical",
        format!(
            "Mount point '{}' is at {:.0}% capacity. Free up space to prevent issues.",
            mount_point, usage_percent
        ),
    )
    .with_priority(NotificationPriority::Urgent)
    .with_icon("drive-harddisk-symbolic");

    send_notification(&notification);
}

/// Creates and sends a notification for high memory pressure.
pub fn notify_memory_pressure(available_mb: u64) {
    let notification = DesktopNotification::new(
        "memory-pressure",
        "High Memory Pressure",
        format!(
            "Available memory is low ({} MB remaining). Consider closing some applications.",
            available_mb
        ),
    )
    .with_priority(NotificationPriority::High)
    .with_icon("dialog-warning-symbolic");

    send_notification(&notification);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_builder() {
        let notif = DesktopNotification::new("test-id", "Test Title", "Test body")
            .with_priority(NotificationPriority::High)
            .with_icon("dialog-warning-symbolic");

        assert_eq!(notif.id, "test-id");
        assert_eq!(notif.title, "Test Title");
        assert_eq!(notif.body, "Test body");
        assert_eq!(notif.priority, NotificationPriority::High);
        assert_eq!(notif.icon, Some("dialog-warning-symbolic".to_string()));
    }

    #[test]
    fn test_default_priority_is_normal() {
        let notif = DesktopNotification::new("id", "title", "body");
        assert_eq!(notif.priority, NotificationPriority::Normal);
        assert!(notif.icon.is_none());
    }

    #[test]
    fn test_priority_to_gio() {
        assert_eq!(NotificationPriority::Low.to_gio(), gio::NotificationPriority::Low);
        assert_eq!(NotificationPriority::Normal.to_gio(), gio::NotificationPriority::Normal);
        assert_eq!(NotificationPriority::High.to_gio(), gio::NotificationPriority::High);
        assert_eq!(NotificationPriority::Urgent.to_gio(), gio::NotificationPriority::Urgent);
    }

    #[test]
    fn test_builder_chaining() {
        let notif = DesktopNotification::new("chain", "T", "B")
            .with_priority(NotificationPriority::Urgent)
            .with_icon("icon-name");
        assert_eq!(notif.priority, NotificationPriority::Urgent);
        assert_eq!(notif.icon.as_deref(), Some("icon-name"));
    }

    #[test]
    fn test_notification_debug_impl() {
        let notif = DesktopNotification::new("d", "Debug Test", "body");
        let debug_str = format!("{:?}", notif);
        assert!(debug_str.contains("Debug Test"));
    }

    #[test]
    fn test_notification_clone() {
        let notif = DesktopNotification::new("c", "Clone", "body")
            .with_icon("icon");
        let cloned = notif.clone();
        assert_eq!(notif.id, cloned.id);
        assert_eq!(notif.icon, cloned.icon);
    }
}
