//! Flatpak application collector.
//!
//! Collects installed Flatpak applications, runtimes, and configured remotes.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{
    FlatpakApp, FlatpakInstallation, FlatpakRemote, FlatpakRuntime, FlatpakState, Snapshot,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use tracing::debug;

/// Collects Flatpak application state.
pub struct FlatpakCollector;

impl SnapshotCollector for FlatpakCollector {
    fn name(&self) -> &'static str {
        "flatpak"
    }

    fn is_available(&self) -> bool {
        Path::new("/usr/bin/flatpak").exists() || Path::new("/bin/flatpak").exists()
    }

    fn collect(&self, snapshot: &mut Snapshot, _redact: bool) -> Result<(), CollectorError> {
        let mut state = FlatpakState::default();

        // Collect system apps
        if let Ok(apps) = collect_apps(FlatpakInstallation::System) {
            for (id, app) in apps {
                state.apps.insert(id, app);
            }
        }

        // Collect user apps
        if let Ok(apps) = collect_apps(FlatpakInstallation::User) {
            for (id, app) in apps {
                state.apps.insert(id, app);
            }
        }

        // Collect runtimes
        if let Ok(runtimes) = collect_runtimes() {
            state.runtimes = runtimes;
        }

        // Collect remotes
        if let Ok(remotes) = collect_remotes() {
            state.remotes = remotes;
        }

        debug!(
            apps = state.apps.len(),
            runtimes = state.runtimes.len(),
            remotes = state.remotes.len(),
            "Collected Flatpak state"
        );

        snapshot.flatpaks = state;
        Ok(())
    }
}

/// Collects installed Flatpak applications.
fn collect_apps(installation: FlatpakInstallation) -> Result<BTreeMap<String, FlatpakApp>, CollectorError> {
    let install_arg = match installation {
        FlatpakInstallation::System => "--system",
        FlatpakInstallation::User => "--user",
    };

    let output = Command::new("flatpak")
        .args([
            "list",
            install_arg,
            "--app",
            "--columns=application,name,version,branch,arch,origin",
        ])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("flatpak list failed: {e}")))?;

    if !output.status.success() {
        // Don't fail if just no apps for this installation type
        return Ok(BTreeMap::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut apps = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 6 {
            let app_id = parts[0].to_string();
            let app = FlatpakApp {
                name: parts[1].to_string(),
                version: parts[2].to_string(),
                branch: parts[3].to_string(),
                arch: parts[4].to_string(),
                origin: parts[5].to_string(),
                installation,
            };
            apps.insert(app_id, app);
        }
    }

    Ok(apps)
}

/// Collects installed Flatpak runtimes.
fn collect_runtimes() -> Result<BTreeMap<String, FlatpakRuntime>, CollectorError> {
    let output = Command::new("flatpak")
        .args([
            "list",
            "--runtime",
            "--columns=application,name,version,branch,arch,origin",
        ])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("flatpak list runtimes failed: {e}")))?;

    if !output.status.success() {
        return Ok(BTreeMap::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut runtimes = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 6 {
            let runtime_id = parts[0].to_string();
            let runtime = FlatpakRuntime {
                name: parts[1].to_string(),
                version: parts[2].to_string(),
                branch: parts[3].to_string(),
                arch: parts[4].to_string(),
                origin: parts[5].to_string(),
            };
            runtimes.insert(runtime_id, runtime);
        }
    }

    Ok(runtimes)
}

/// Collects configured Flatpak remotes.
fn collect_remotes() -> Result<Vec<FlatpakRemote>, CollectorError> {
    let mut remotes = Vec::new();

    // Collect system remotes
    if let Ok(system_remotes) = collect_remotes_for_installation(FlatpakInstallation::System) {
        remotes.extend(system_remotes);
    }

    // Collect user remotes
    if let Ok(user_remotes) = collect_remotes_for_installation(FlatpakInstallation::User) {
        remotes.extend(user_remotes);
    }

    Ok(remotes)
}

/// Collects remotes for a specific installation type.
fn collect_remotes_for_installation(
    installation: FlatpakInstallation,
) -> Result<Vec<FlatpakRemote>, CollectorError> {
    let install_arg = match installation {
        FlatpakInstallation::System => "--system",
        FlatpakInstallation::User => "--user",
    };

    let output = Command::new("flatpak")
        .args(["remotes", install_arg, "--columns=name,url"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("flatpak remotes failed: {e}")))?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut remotes = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if !parts.is_empty() {
            let remote = FlatpakRemote {
                name: parts[0].to_string(),
                url: parts.get(1).map(|s| s.to_string()),
                installation,
            };
            remotes.push(remote);
        }
    }

    Ok(remotes)
}
