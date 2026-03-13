//! Systemd snapshot collector.
//!
//! Collects enabled/disabled state of system and user units.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{ActiveState, EnablementState, Snapshot, SystemdState, UnitState};
use std::collections::BTreeMap;
use std::process::Command;
use tracing::debug;

/// Collects systemd unit state information.
pub struct SystemdSnapshotCollector;

impl SnapshotCollector for SystemdSnapshotCollector {
    fn name(&self) -> &'static str {
        "systemd"
    }

    fn is_available(&self) -> bool {
        Command::new("systemctl")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn collect(&self, snapshot: &mut Snapshot, _redact: bool) -> Result<(), CollectorError> {
        // Collect system units
        let system_units = collect_units(false)?;
        debug!(count = system_units.len(), "Collected system units");

        // Collect user units
        let user_units = collect_units(true).unwrap_or_default();
        debug!(count = user_units.len(), "Collected user units");

        snapshot.systemd = SystemdState {
            system_units,
            user_units,
        };

        Ok(())
    }
}

/// Collects units from systemctl.
fn collect_units(user: bool) -> Result<BTreeMap<String, UnitState>, CollectorError> {
    let mut cmd = Command::new("systemctl");

    if user {
        cmd.arg("--user");
    }

    cmd.args([
        "list-unit-files",
        "--no-legend",
        "--no-pager",
        "--full",
    ]);

    let output = cmd
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("systemctl failed: {e}")))?;

    if !output.status.success() {
        // User session might not be available (e.g., in containers)
        if user {
            return Ok(BTreeMap::new());
        }
        return Err(CollectorError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut units = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let unit_name = parts[0].to_string();
            let enabled_str = parts[1];

            // Determine unit type from extension
            let unit_type = unit_name
                .rsplit('.')
                .next()
                .unwrap_or("unknown")
                .to_string();

            let enabled = parse_enablement_state(enabled_str);

            units.insert(
                unit_name,
                UnitState {
                    unit_type,
                    enabled,
                    active_state: ActiveState::Unknown, // Will be filled by active state query
                    has_overrides: false,
                    description: None,
                },
            );
        }
    }

    // Now get active states
    let active_states = collect_active_states(user)?;
    for (name, state) in &mut units {
        if let Some(active) = active_states.get(name) {
            state.active_state = *active;
        }
    }

    // Check for drop-in overrides
    check_overrides(&mut units, user);

    Ok(units)
}

/// Collects active states for units.
fn collect_active_states(user: bool) -> Result<BTreeMap<String, ActiveState>, CollectorError> {
    let mut cmd = Command::new("systemctl");

    if user {
        cmd.arg("--user");
    }

    cmd.args([
        "list-units",
        "--no-legend",
        "--no-pager",
        "--all",
        "--full",
    ]);

    let output = cmd.output().map_err(|e| {
        CollectorError::CommandFailed(format!("systemctl list-units failed: {e}"))
    })?;

    if !output.status.success() {
        return Ok(BTreeMap::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut states = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Format: UNIT LOAD ACTIVE SUB DESCRIPTION...
        if parts.len() >= 4 {
            let unit_name = parts[0].to_string();
            let active_str = parts[2];
            states.insert(unit_name, parse_active_state(active_str));
        }
    }

    Ok(states)
}

/// Checks for drop-in overrides.
fn check_overrides(units: &mut BTreeMap<String, UnitState>, user: bool) {
    let mut cmd = Command::new("systemctl");

    if user {
        cmd.arg("--user");
    }

    cmd.args(["show", "--property=DropInPaths", "*"]);

    // This is a best-effort check; we don't fail if it doesn't work
    if let Ok(output) = cmd.output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Parse output and mark units with overrides
            // Format varies by systemd version, so we do a simple check
            for line in stdout.lines() {
                if line.contains("DropInPaths=") && line.contains(".d/") {
                    // Extract unit name from the path
                    // This is approximate; more robust parsing would be needed
                    if let Some(unit) = line
                        .split(".d/")
                        .next()
                        .and_then(|s| s.rsplit('/').next())
                    {
                        if let Some(state) = units.get_mut(unit) {
                            state.has_overrides = true;
                        }
                    }
                }
            }
        }
    }
}

/// Parses enablement state string.
fn parse_enablement_state(s: &str) -> EnablementState {
    match s {
        "enabled" | "enabled-runtime" => EnablementState::Enabled,
        "disabled" => EnablementState::Disabled,
        "static" => EnablementState::Static,
        "masked" | "masked-runtime" => EnablementState::Masked,
        "indirect" => EnablementState::Indirect,
        "generated" => EnablementState::Generated,
        "alias" => EnablementState::Alias,
        "transient" => EnablementState::Transient,
        _ => EnablementState::Unknown,
    }
}

/// Parses active state string.
fn parse_active_state(s: &str) -> ActiveState {
    match s {
        "active" | "reloading" => ActiveState::Active,
        "inactive" => ActiveState::Inactive,
        "failed" => ActiveState::Failed,
        "activating" => ActiveState::Activating,
        "deactivating" => ActiveState::Deactivating,
        _ => ActiveState::Unknown,
    }
}
