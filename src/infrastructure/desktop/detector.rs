//! Desktop environment detection.
//!
//! Detects GNOME, KDE Plasma, COSMIC, and other desktops.

use serde::{Deserialize, Serialize};
use std::env;
use tracing::debug;

/// Detected desktop environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesktopEnvironment {
    /// GNOME desktop.
    Gnome,
    /// KDE Plasma desktop.
    KdePlasma,
    /// System76 COSMIC desktop.
    Cosmic,
    /// Xfce desktop.
    Xfce,
    /// Cinnamon desktop (Linux Mint).
    Cinnamon,
    /// MATE desktop.
    Mate,
    /// LXQt desktop.
    Lxqt,
    /// Budgie desktop.
    Budgie,
    /// Pantheon desktop (elementary OS).
    Pantheon,
    /// Unknown or undetected desktop.
    Unknown,
}

impl DesktopEnvironment {
    /// Returns a human-readable name for this desktop.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Gnome => "GNOME",
            Self::KdePlasma => "KDE Plasma",
            Self::Cosmic => "COSMIC",
            Self::Xfce => "Xfce",
            Self::Cinnamon => "Cinnamon",
            Self::Mate => "MATE",
            Self::Lxqt => "LXQt",
            Self::Budgie => "Budgie",
            Self::Pantheon => "Pantheon",
            Self::Unknown => "Unknown",
        }
    }

    /// Returns true if this desktop is known to support libadwaita well.
    #[must_use]
    pub const fn supports_libadwaita(&self) -> bool {
        matches!(self, Self::Gnome | Self::Cosmic | Self::Budgie | Self::Pantheon)
    }
}

/// Session type (display server).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    /// Wayland session.
    Wayland,
    /// X11 session.
    X11,
    /// TTY (no graphical session).
    Tty,
    /// Unknown session type.
    Unknown,
}

impl SessionType {
    /// Returns a human-readable name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Wayland => "Wayland",
            Self::X11 => "X11",
            Self::Tty => "TTY",
            Self::Unknown => "Unknown",
        }
    }
}

/// Detected desktop information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopInfo {
    /// The desktop environment.
    pub environment: DesktopEnvironment,

    /// The session type (Wayland/X11).
    pub session_type: SessionType,

    /// Raw XDG_CURRENT_DESKTOP value.
    pub xdg_current_desktop: Option<String>,

    /// Raw XDG_SESSION_TYPE value.
    pub xdg_session_type: Option<String>,

    /// Raw DESKTOP_SESSION value.
    pub desktop_session: Option<String>,
}

/// Detects the current desktop environment and session type.
#[must_use]
pub fn detect_desktop() -> DesktopInfo {
    let xdg_current_desktop = env::var("XDG_CURRENT_DESKTOP").ok();
    let xdg_session_type = env::var("XDG_SESSION_TYPE").ok();
    let desktop_session = env::var("DESKTOP_SESSION").ok();

    debug!(
        xdg_current_desktop = ?xdg_current_desktop,
        xdg_session_type = ?xdg_session_type,
        desktop_session = ?desktop_session,
        "Detecting desktop environment"
    );

    let environment = detect_environment(&xdg_current_desktop, &desktop_session);
    let session_type = detect_session_type(&xdg_session_type);

    debug!(
        environment = ?environment,
        session_type = ?session_type,
        "Desktop detected"
    );

    DesktopInfo {
        environment,
        session_type,
        xdg_current_desktop,
        xdg_session_type,
        desktop_session,
    }
}

fn detect_environment(
    xdg_current_desktop: &Option<String>,
    desktop_session: &Option<String>,
) -> DesktopEnvironment {
    // XDG_CURRENT_DESKTOP can contain multiple values separated by ':'
    let current_desktop = xdg_current_desktop
        .as_ref()
        .map(|s| s.to_uppercase())
        .unwrap_or_default();

    // Check for specific desktops
    if current_desktop.contains("GNOME") {
        return DesktopEnvironment::Gnome;
    }
    if current_desktop.contains("KDE") || current_desktop.contains("PLASMA") {
        return DesktopEnvironment::KdePlasma;
    }
    if current_desktop.contains("COSMIC") {
        return DesktopEnvironment::Cosmic;
    }
    if current_desktop.contains("XFCE") {
        return DesktopEnvironment::Xfce;
    }
    if current_desktop.contains("CINNAMON") || current_desktop.contains("X-CINNAMON") {
        return DesktopEnvironment::Cinnamon;
    }
    if current_desktop.contains("MATE") {
        return DesktopEnvironment::Mate;
    }
    if current_desktop.contains("LXQT") {
        return DesktopEnvironment::Lxqt;
    }
    if current_desktop.contains("BUDGIE") {
        return DesktopEnvironment::Budgie;
    }
    if current_desktop.contains("PANTHEON") {
        return DesktopEnvironment::Pantheon;
    }

    // Fall back to DESKTOP_SESSION
    if let Some(session) = desktop_session {
        let session_upper = session.to_uppercase();
        if session_upper.contains("GNOME") {
            return DesktopEnvironment::Gnome;
        }
        if session_upper.contains("PLASMA") || session_upper.contains("KDE") {
            return DesktopEnvironment::KdePlasma;
        }
    }

    DesktopEnvironment::Unknown
}

fn detect_session_type(xdg_session_type: &Option<String>) -> SessionType {
    match xdg_session_type.as_deref() {
        Some("wayland") => SessionType::Wayland,
        Some("x11") => SessionType::X11,
        Some("tty") => SessionType::Tty,
        _ => SessionType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gnome_detection() {
        let env = detect_environment(&Some("GNOME".to_string()), &None);
        assert_eq!(env, DesktopEnvironment::Gnome);

        let env = detect_environment(&Some("ubuntu:GNOME".to_string()), &None);
        assert_eq!(env, DesktopEnvironment::Gnome);
    }

    #[test]
    fn test_kde_detection() {
        let env = detect_environment(&Some("KDE".to_string()), &None);
        assert_eq!(env, DesktopEnvironment::KdePlasma);

        let env = detect_environment(&Some("plasma".to_string()), &None);
        assert_eq!(env, DesktopEnvironment::KdePlasma);
    }

    #[test]
    fn test_cosmic_detection() {
        let env = detect_environment(&Some("COSMIC".to_string()), &None);
        assert_eq!(env, DesktopEnvironment::Cosmic);
    }

    #[test]
    fn test_session_type_detection() {
        assert_eq!(
            detect_session_type(&Some("wayland".to_string())),
            SessionType::Wayland
        );
        assert_eq!(
            detect_session_type(&Some("x11".to_string())),
            SessionType::X11
        );
    }

    #[test]
    fn test_display_names() {
        assert_eq!(DesktopEnvironment::Gnome.display_name(), "GNOME");
        assert_eq!(SessionType::Wayland.display_name(), "Wayland");
    }
}
