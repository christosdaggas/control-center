//! Icon resolution with 4-step fallback.
//!
//! Ensures icons never fail to load by following a strict fallback chain.

use tracing::{debug, warn};

/// Icon resolution result.
#[derive(Debug, Clone)]
pub struct ResolvedIcon {
    /// The icon name that was successfully resolved.
    pub name: String,

    /// Whether this is the originally requested icon.
    pub is_original: bool,

    /// Whether this is a bundled fallback.
    pub is_bundled_fallback: bool,

    /// Whether this is the final generic fallback.
    pub is_generic_fallback: bool,
}

/// Icon resolver with 4-step fallback strategy.
///
/// Steps:
/// 1. Load icon by standard Freedesktop name from system theme
/// 2. Try fallback names (documented alternatives)
/// 3. Fall back to bundled minimal icons
/// 4. Final fallback to generic "application" icon
pub struct IconResolver {
    /// Mapping of icon names to their fallbacks.
    fallback_map: std::collections::HashMap<&'static str, Vec<&'static str>>,

    /// The generic fallback icon (always available).
    generic_fallback: &'static str,
}

impl IconResolver {
    /// Creates a new icon resolver.
    #[must_use]
    pub fn new() -> Self {
        let mut fallback_map = std::collections::HashMap::new();

        // Define fallbacks for common icons
        fallback_map.insert(
            "package-x-generic-symbolic",
            vec!["package-x-generic", "package", "application-x-addon-symbolic"],
        );
        fallback_map.insert(
            "system-run-symbolic",
            vec!["system-run", "utilities-terminal-symbolic", "utilities-terminal"],
        );
        fallback_map.insert(
            "dialog-error-symbolic",
            vec!["dialog-error", "error", "emblem-important-symbolic"],
        );
        fallback_map.insert(
            "dialog-warning-symbolic",
            vec!["dialog-warning", "warning", "emblem-warning"],
        );
        fallback_map.insert(
            "dialog-information-symbolic",
            vec!["dialog-information", "info", "help-about-symbolic"],
        );
        fallback_map.insert(
            "network-offline-symbolic",
            vec!["network-offline", "network-error-symbolic", "network-wired-disconnected"],
        );
        fallback_map.insert(
            "network-wired-symbolic",
            vec!["network-wired", "network-workgroup-symbolic", "network-server-symbolic"],
        );
        fallback_map.insert(
            "drive-harddisk-symbolic",
            vec!["drive-harddisk", "drive-removable-media-symbolic", "folder"],
        );
        fallback_map.insert(
            "dialog-password-symbolic",
            vec!["dialog-password", "system-lock-screen-symbolic", "security-high-symbolic"],
        );
        fallback_map.insert(
            "security-high-symbolic",
            vec!["security-high", "changes-prevent-symbolic", "dialog-password-symbolic"],
        );
        fallback_map.insert(
            "computer-symbolic",
            vec!["computer", "user-desktop-symbolic", "preferences-system-symbolic"],
        );
        fallback_map.insert(
            "system-reboot-symbolic",
            vec!["system-reboot", "view-refresh-symbolic", "emblem-synchronizing-symbolic"],
        );
        fallback_map.insert(
            "system-shutdown-symbolic",
            vec!["system-shutdown", "system-log-out-symbolic", "application-exit-symbolic"],
        );

        Self {
            fallback_map,
            generic_fallback: "application-x-executable-symbolic",
        }
    }

    /// Resolves an icon name using the 4-step fallback strategy.
    ///
    /// Returns the resolved icon information.
    #[must_use]
    pub fn resolve(&self, icon_name: &str) -> ResolvedIcon {
        let icon_theme = gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap());

        // Step 1: Try the original icon name
        if icon_theme.has_icon(icon_name) {
            debug!(icon = icon_name, "Icon found in theme");
            return ResolvedIcon {
                name: icon_name.to_string(),
                is_original: true,
                is_bundled_fallback: false,
                is_generic_fallback: false,
            };
        }

        // Step 2: Try fallback names
        if let Some(fallbacks) = self.fallback_map.get(icon_name) {
            for fallback in fallbacks {
                if icon_theme.has_icon(fallback) {
                    debug!(
                        original = icon_name,
                        fallback = fallback,
                        "Using fallback icon"
                    );
                    return ResolvedIcon {
                        name: (*fallback).to_string(),
                        is_original: false,
                        is_bundled_fallback: false,
                        is_generic_fallback: false,
                    };
                }
            }
        }

        // Step 3: Try bundled icons (would be in app resources)
        // For now, we skip to step 4 as bundled icons require GResource setup

        // Step 4: Final fallback to generic icon
        warn!(
            icon = icon_name,
            fallback = self.generic_fallback,
            "Using generic fallback icon"
        );
        
        let final_name = if icon_theme.has_icon(self.generic_fallback) {
            self.generic_fallback.to_string()
        } else {
            // Absolute last resort
            "image-missing".to_string()
        };

        ResolvedIcon {
            name: final_name,
            is_original: false,
            is_bundled_fallback: false,
            is_generic_fallback: true,
        }
    }

    /// Convenience method to get just the icon name.
    #[must_use]
    pub fn resolve_name(&self, icon_name: &str) -> String {
        self.resolve(icon_name).name
    }

    /// Checks if an icon would need fallback resolution.
    #[must_use]
    pub fn needs_fallback(&self, icon_name: &str) -> bool {
        let icon_theme = gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap());
        !icon_theme.has_icon(icon_name)
    }

    /// Returns the fallback map for diagnostics.
    #[must_use]
    pub fn fallback_map(&self) -> &std::collections::HashMap<&'static str, Vec<&'static str>> {
        &self.fallback_map
    }
}

impl Default for IconResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_creation() {
        let resolver = IconResolver::new();
        assert!(!resolver.fallback_map.is_empty());
    }

    #[test]
    fn test_fallback_map_has_common_icons() {
        let resolver = IconResolver::new();
        assert!(resolver.fallback_map.contains_key("dialog-error-symbolic"));
        assert!(resolver.fallback_map.contains_key("network-offline-symbolic"));
    }
}
