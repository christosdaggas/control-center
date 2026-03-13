//! Theme detection and management.

use crate::config::ThemePreference;
use tracing::debug;

/// Represents the current system color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// Light color scheme.
    Light,
    /// Dark color scheme.
    Dark,
}

/// Detects the system color scheme preference.
#[must_use]
pub fn detect_color_scheme() -> ColorScheme {
    // Try GTK settings first
    if let Some(scheme) = detect_gtk_color_scheme() {
        return scheme;
    }

    // Fall back to environment variable
    if let Ok(theme) = std::env::var("GTK_THEME") {
        if theme.to_lowercase().contains("dark") {
            return ColorScheme::Dark;
        }
    }

    // Default to light
    debug!("Could not detect color scheme, defaulting to light");
    ColorScheme::Light
}

fn detect_gtk_color_scheme() -> Option<ColorScheme> {
    // Check gsettings for GNOME
    let output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let scheme = stdout.trim().trim_matches('\'');

    debug!(scheme = scheme, "Detected GTK color scheme");

    if scheme.contains("dark") {
        Some(ColorScheme::Dark)
    } else {
        Some(ColorScheme::Light)
    }
}

/// Resolves the effective color scheme based on user preference.
#[must_use]
pub fn resolve_color_scheme(preference: ThemePreference) -> ColorScheme {
    match preference {
        ThemePreference::System => detect_color_scheme(),
        ThemePreference::Light => ColorScheme::Light,
        ThemePreference::Dark => ColorScheme::Dark,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_explicit_preference() {
        assert_eq!(
            resolve_color_scheme(ThemePreference::Light),
            ColorScheme::Light
        );
        assert_eq!(
            resolve_color_scheme(ThemePreference::Dark),
            ColorScheme::Dark
        );
    }
}
