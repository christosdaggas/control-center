//! Systemd adapter for reading and controlling services.
//!
//! Provides unified access to system and user units via systemctl.

use std::collections::HashMap;
use std::process::Command;
use tracing::debug;

/// Represents a systemd unit type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitType {
    /// A service unit.
    Service,
    /// A timer unit.
    Timer,
    /// A socket unit.
    Socket,
    /// A target unit.
    Target,
    /// A mount unit.
    Mount,
    /// A path unit.
    Path,
    /// A scope unit.
    Scope,
    /// A slice unit.
    Slice,
    /// A device unit.
    Device,
    /// An automount unit.
    Automount,
    /// A swap unit.
    Swap,
    /// Unknown unit type.
    Unknown,
}

impl UnitType {
    /// Parses a unit type from its file extension.
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "service" => Self::Service,
            "timer" => Self::Timer,
            "socket" => Self::Socket,
            "target" => Self::Target,
            "mount" => Self::Mount,
            "path" => Self::Path,
            "scope" => Self::Scope,
            "slice" => Self::Slice,
            "device" => Self::Device,
            "automount" => Self::Automount,
            "swap" => Self::Swap,
            _ => Self::Unknown,
        }
    }

    /// Returns the icon name for this unit type.
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Service => "system-run-symbolic",
            Self::Timer => "alarm-symbolic",
            Self::Socket => "network-transmit-receive-symbolic",
            Self::Target => "emblem-system-symbolic",
            Self::Mount => "drive-harddisk-symbolic",
            Self::Path => "folder-symbolic",
            Self::Scope => "view-grid-symbolic",
            Self::Slice => "view-continuous-symbolic",
            Self::Device => "drive-harddisk-usb-symbolic",
            Self::Automount => "media-removable-symbolic",
            Self::Swap => "media-memory-symbolic",
            Self::Unknown => "dialog-question-symbolic",
        }
    }
}

/// State of a systemd unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitState {
    /// Unit is running.
    Active,
    /// Unit is not running but can be started.
    Inactive,
    /// Unit has failed.
    Failed,
    /// Unit is in the process of starting.
    Activating,
    /// Unit is in the process of stopping.
    Deactivating,
    /// Unit is being reloaded.
    Reloading,
    /// Unit state could not be determined.
    Unknown,
}

impl UnitState {
    /// Parses a unit state from systemctl output.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "active" => Self::Active,
            "inactive" => Self::Inactive,
            "failed" => Self::Failed,
            "activating" => Self::Activating,
            "deactivating" => Self::Deactivating,
            "reloading" => Self::Reloading,
            _ => Self::Unknown,
        }
    }

    /// Returns the CSS class for this state.
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Active => "success",
            Self::Inactive => "dim-label",
            Self::Failed => "error",
            Self::Activating | Self::Deactivating | Self::Reloading => "warning",
            Self::Unknown => "dim-label",
        }
    }

    /// Returns the display name for this state.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Active => "Running",
            Self::Inactive => "Stopped",
            Self::Failed => "Failed",
            Self::Activating => "Starting...",
            Self::Deactivating => "Stopping...",
            Self::Reloading => "Reloading...",
            Self::Unknown => "Unknown",
        }
    }
}

/// Enabled/disabled state of a unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnabledState {
    /// Unit is enabled (starts at boot).
    Enabled,
    /// Unit is disabled.
    Disabled,
    /// Unit is statically configured.
    Static,
    /// Unit is masked (cannot be started).
    Masked,
    /// Unit is an alias for another.
    Alias,
    /// Unit is indirectly enabled.
    Indirect,
    /// Unit was generated dynamically.
    Generated,
    /// Unit is transient.
    Transient,
    /// Unknown enabled state.
    Unknown,
}

impl EnabledState {
    /// Parses from systemctl output.
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "enabled" => Self::Enabled,
            "disabled" => Self::Disabled,
            "static" => Self::Static,
            "masked" => Self::Masked,
            "alias" => Self::Alias,
            "indirect" => Self::Indirect,
            "generated" => Self::Generated,
            "transient" => Self::Transient,
            _ => Self::Unknown,
        }
    }

    /// Whether this unit can be toggled.
    pub fn can_toggle(&self) -> bool {
        matches!(self, Self::Enabled | Self::Disabled)
    }

    /// Display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Enabled => "Enabled",
            Self::Disabled => "Disabled",
            Self::Static => "Static",
            Self::Masked => "Masked",
            Self::Alias => "Alias",
            Self::Indirect => "Indirect",
            Self::Generated => "Generated",
            Self::Transient => "Transient",
            Self::Unknown => "Unknown",
        }
    }
}

/// A systemd unit with its current status.
#[derive(Debug, Clone)]
pub struct SystemdUnit {
    /// Full unit name (e.g., "nginx.service").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Type of unit.
    pub unit_type: UnitType,
    /// Current running state.
    pub state: UnitState,
    /// Whether it's enabled at boot.
    pub enabled: EnabledState,
    /// Whether this is a user unit (vs system).
    pub is_user: bool,
    /// Load state (loaded, not-found, etc.).
    pub load_state: String,
}

impl SystemdUnit {
    /// Returns true if this is a critical system unit.
    pub fn is_critical(&self) -> bool {
        let critical_patterns = [
            "systemd-",
            "dbus",
            "NetworkManager",
            "polkit",
            "udev",
            "journald",
            "logind",
        ];
        critical_patterns.iter().any(|p| self.name.contains(p))
    }

    /// Returns the short name without the type suffix.
    pub fn short_name(&self) -> &str {
        self.name.split('.').next().unwrap_or(&self.name)
    }
}

/// Adapter for interacting with systemd.
pub struct SystemdAdapter;

impl SystemdAdapter {
    /// Lists all units of a given type, including those not currently loaded.
    pub fn list_units(unit_type: Option<UnitType>, user: bool) -> Vec<SystemdUnit> {
        let type_filter = match unit_type {
            Some(UnitType::Service) => Some("service"),
            Some(UnitType::Timer) => Some("timer"),
            Some(UnitType::Socket) => Some("socket"),
            Some(UnitType::Target) => Some("target"),
            Some(UnitType::Mount) => Some("mount"),
            Some(_) => return vec![],
            None => None,
        };

        // Get loaded units first
        let mut cmd = Command::new("systemctl");
        if user {
            cmd.arg("--user");
        }
        cmd.args(["list-units", "--all", "--no-pager", "--plain", "--no-legend"]);
        if let Some(tf) = type_filter {
            cmd.args(["--type", tf]);
        }

        debug!(user = user, unit_type = ?unit_type, "Listing systemd units");

        let mut units: HashMap<String, SystemdUnit> = HashMap::new();

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for unit in Self::parse_list_units_output(&stdout, user) {
                    units.insert(unit.name.clone(), unit);
                }
            }
        }

        // Also get unit files to include unloaded services
        let mut cmd2 = Command::new("systemctl");
        if user {
            cmd2.arg("--user");
        }
        cmd2.args(["list-unit-files", "--no-pager", "--plain", "--no-legend"]);
        if let Some(tf) = type_filter {
            cmd2.args(["--type", tf]);
        }

        if let Ok(output) = cmd2.output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let enabled_state = EnabledState::from_str(parts[1]);
                        
                        // Only add if not already in units map
                        if !units.contains_key(&name) {
                            let unit_t = name
                                .rsplit('.')
                                .next()
                                .map(UnitType::from_extension)
                                .unwrap_or(UnitType::Unknown);
                            
                            units.insert(name.clone(), SystemdUnit {
                                name,
                                description: String::new(),
                                unit_type: unit_t,
                                state: UnitState::Inactive,
                                enabled: enabled_state,
                                is_user: user,
                                load_state: "not-loaded".to_string(),
                            });
                        }
                    }
                }
            }
        }

        let mut result: Vec<_> = units.into_values().collect();
        
        // Fill in enabled states for loaded units
        Self::fill_enabled_states(&mut result, user);
        
        // Sort by name
        result.sort_by(|a, b| a.name.cmp(&b.name));
        
        result
    }

    /// Parses the output of `systemctl list-units`.
    fn parse_list_units_output(output: &str, is_user: bool) -> Vec<SystemdUnit> {
        let mut units = Vec::new();

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            let name = parts[0].to_string();
            let load_state = parts[1].to_string();
            let active = parts[2];
            let _sub = parts[3]; // sub-state, not used currently
            let description = if parts.len() > 4 {
                parts[4..].join(" ")
            } else {
                String::new()
            };

            // Determine unit type from name
            let unit_type = name
                .rsplit('.')
                .next()
                .map(UnitType::from_extension)
                .unwrap_or(UnitType::Unknown);

            units.push(SystemdUnit {
                name,
                description,
                unit_type,
                state: UnitState::from_str(active),
                enabled: EnabledState::Unknown, // Will be filled in separately
                is_user,
                load_state,
            });
        }

        // Batch get enabled states
        Self::fill_enabled_states(&mut units, is_user);

        units
    }

    /// Fills in the enabled state for a list of units.
    fn fill_enabled_states(units: &mut [SystemdUnit], user: bool) {
        if units.is_empty() {
            return;
        }

        let mut cmd = Command::new("systemctl");
        if user {
            cmd.arg("--user");
        }
        cmd.args(["list-unit-files", "--no-pager", "--plain", "--no-legend"]);

        let output = match cmd.output() {
            Ok(o) => o,
            Err(_) => return,
        };

        if !output.status.success() {
            return;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let enabled_map: HashMap<String, EnabledState> = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    Some((parts[0].to_string(), EnabledState::from_str(parts[1])))
                } else {
                    None
                }
            })
            .collect();

        for unit in units.iter_mut() {
            if let Some(state) = enabled_map.get(&unit.name) {
                unit.enabled = *state;
            }
        }
    }

    /// Gets the count of failed units.
    pub fn failed_count(user: bool) -> usize {
        let mut cmd = Command::new("systemctl");
        if user {
            cmd.arg("--user");
        }
        cmd.args(["--failed", "--plain", "--no-legend", "--no-pager"]);

        let output = match cmd.output() {
            Ok(o) => o,
            Err(_) => return 0,
        };

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    }

    /// Starts a unit.
    pub fn start(unit_name: &str, user: bool) -> Result<(), String> {
        Self::run_action("start", unit_name, user)
    }

    /// Stops a unit.
    pub fn stop(unit_name: &str, user: bool) -> Result<(), String> {
        Self::run_action("stop", unit_name, user)
    }

    /// Restarts a unit.
    pub fn restart(unit_name: &str, user: bool) -> Result<(), String> {
        Self::run_action("restart", unit_name, user)
    }

    /// Enables a unit at boot.
    pub fn enable(unit_name: &str, user: bool) -> Result<(), String> {
        Self::run_action("enable", unit_name, user)
    }

    /// Disables a unit at boot.
    pub fn disable(unit_name: &str, user: bool) -> Result<(), String> {
        Self::run_action("disable", unit_name, user)
    }

    /// Runs a systemctl action on a unit.
    fn run_action(action: &str, unit_name: &str, user: bool) -> Result<(), String> {
        let mut cmd = Command::new("systemctl");
        if user {
            cmd.arg("--user");
        }
        cmd.args([action, unit_name]);

        debug!(action = action, unit = unit_name, user = user, "Running systemctl action");

        let output = cmd.output().map_err(|e| format!("Failed to run systemctl: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("systemctl {} failed: {}", action, stderr.trim()))
        }
    }

    /// Gets recent logs for a unit.
    pub fn get_unit_logs(unit_name: &str, user: bool, lines: usize) -> Vec<String> {
        let mut cmd = Command::new("journalctl");
        if user {
            cmd.arg("--user");
        }
        cmd.args([
            "-u",
            unit_name,
            "-n",
            &lines.to_string(),
            "--no-pager",
            "-o",
            "short-iso",
        ]);

        let output = match cmd.output() {
            Ok(o) => o,
            Err(_) => return vec![],
        };

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(String::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unit_type_from_extension() {
        assert_eq!(UnitType::from_extension("service"), UnitType::Service);
        assert_eq!(UnitType::from_extension("timer"), UnitType::Timer);
        assert_eq!(UnitType::from_extension("foo"), UnitType::Unknown);
    }

    #[test]
    fn test_unit_state_from_str() {
        assert_eq!(UnitState::from_str("active"), UnitState::Active);
        assert_eq!(UnitState::from_str("FAILED"), UnitState::Failed);
        assert_eq!(UnitState::from_str("whatever"), UnitState::Unknown);
    }

    #[test]
    fn test_enabled_state_can_toggle() {
        assert!(EnabledState::Enabled.can_toggle());
        assert!(EnabledState::Disabled.can_toggle());
        assert!(!EnabledState::Static.can_toggle());
        assert!(!EnabledState::Masked.can_toggle());
    }
}
