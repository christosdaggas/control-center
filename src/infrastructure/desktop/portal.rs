//! XDG Desktop Portal integration.
//!
//! Uses xdg-desktop-portal for sandboxed file operations.

use std::process::Command;
use tracing::debug;

/// Checks if xdg-desktop-portal is available.
#[must_use]
pub fn is_portal_available() -> bool {
    // Check if the portal service is running
    let result = Command::new("busctl")
        .args([
            "--user",
            "introspect",
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
        ])
        .output();

    match result {
        Ok(output) => {
            let available = output.status.success();
            debug!(available = available, "XDG desktop portal check");
            available
        }
        Err(_) => {
            debug!("busctl not available, cannot check portal status");
            false
        }
    }
}

/// Information about available portal capabilities.
#[derive(Debug, Clone)]
pub struct PortalCapabilities {
    /// Whether the portal is available at all.
    pub available: bool,

    /// Whether file chooser is available.
    pub file_chooser: bool,

    /// Whether notifications are available.
    pub notifications: bool,

    /// Whether settings (dark mode) are available.
    pub settings: bool,
}

/// Detects available portal capabilities.
#[must_use]
pub fn detect_capabilities() -> PortalCapabilities {
    let available = is_portal_available();

    if !available {
        return PortalCapabilities {
            available: false,
            file_chooser: false,
            notifications: false,
            settings: false,
        };
    }

    // For now, assume all capabilities if portal is available
    // In a real implementation, we'd introspect specific interfaces
    PortalCapabilities {
        available: true,
        file_chooser: true,
        notifications: true,
        settings: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_capabilities_returns_struct() {
        let caps = detect_capabilities();
        // Just verify it returns without error
        assert_eq!(caps.available, caps.file_chooser || !caps.available);
    }
}
