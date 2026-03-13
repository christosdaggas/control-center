//! Application configuration management.
//!
//! Handles loading, saving, and accessing user preferences using XDG paths.

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during configuration operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Failed to determine XDG directories.
    #[error("Could not determine XDG config directory")]
    NoConfigDir,

    /// Failed to read configuration file.
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    /// Failed to parse configuration file.
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_json::Error),
}

/// Theme preference for the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    /// Follow system theme (default).
    #[default]
    System,
    /// Always use light theme.
    Light,
    /// Always use dark theme.
    Dark,
}

/// Filter preset for the timeline view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DefaultFilterPreset {
    /// Show all events.
    #[default]
    All,
    /// Show events since last reboot.
    SinceLastReboot,
    /// Show only warnings and errors.
    WarningsAndErrors,
    /// Show only changes (package updates, config changes).
    ChangesOnly,
}

/// Application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Theme preference.
    pub theme: ThemePreference,

    /// Default filter preset when opening the app.
    pub default_filter: DefaultFilterPreset,

    /// Number of days of history to load by default.
    pub default_history_days: u32,

    /// Whether to show the diagnostics button in the header.
    pub show_diagnostics_button: bool,

    /// Enable diagnostic/verbose logging mode.
    pub diagnostic_mode: bool,

    /// Data retention period in days (0 = keep forever).
    pub data_retention_days: u32,

    /// Whether desktop notifications are enabled.
    pub notifications_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemePreference::default(),
            default_filter: DefaultFilterPreset::default(),
            default_history_days: 7,
            show_diagnostics_button: true,
            diagnostic_mode: false,
            data_retention_days: 90,
            notifications_enabled: false,
        }
    }
}

impl Config {
    /// Returns the path to the configuration file.
    ///
    /// Uses XDG_CONFIG_HOME/control-center/config.json
    pub fn config_path() -> Result<PathBuf, ConfigError> {
        let dirs = ProjectDirs::from("com", "chrisdaggas", "ControlCenter")
            .ok_or(ConfigError::NoConfigDir)?;
        Ok(dirs.config_dir().join("config.json"))
    }

    /// Loads configuration from the default path.
    ///
    /// Returns default configuration if the file doesn't exist.
    pub fn load() -> Result<Self, ConfigError> {
        let path = Self::config_path()?;

        if !path.exists() {
            debug!(?path, "Config file not found, using defaults");
            return Ok(Self::default());
        }

        debug!(?path, "Loading configuration");
        let contents = std::fs::read_to_string(&path)?;
        let config: Self = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Saves configuration to the default path.
    pub fn save(&self) -> Result<(), ConfigError> {
        let path = Self::config_path()?;


        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!(?path, error = %e, "Failed to create config directory");
                return Err(ConfigError::ReadError(e));
            }
        }

        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        debug!(?path, "Configuration saved");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.theme, ThemePreference::System);
        assert_eq!(config.default_history_days, 7);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let json = serde_json::to_string(&config).expect("serialization should succeed");
        let parsed: Config = serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(parsed.theme, config.theme);
    }
}
