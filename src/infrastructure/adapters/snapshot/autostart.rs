//! Autostart collector.
//!
//! Collects XDG autostart entries and user timers.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{AutostartEntry, AutostartState, Snapshot, TimerInfo};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::debug;

/// Collects autostart and timer information.
pub struct AutostartCollector;

impl SnapshotCollector for AutostartCollector {
    fn name(&self) -> &'static str {
        "autostart"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self, snapshot: &mut Snapshot, _redact: bool) -> Result<(), CollectorError> {
        let desktop_entries = collect_autostart_entries()?;
        debug!(count = desktop_entries.len(), "Collected autostart entries");

        let user_timers = collect_user_timers().unwrap_or_default();
        debug!(count = user_timers.len(), "Collected user timers");

        snapshot.autostart = AutostartState {
            desktop_entries,
            user_timers,
        };

        Ok(())
    }
}

/// Collects XDG autostart desktop entries.
fn collect_autostart_entries() -> Result<Vec<AutostartEntry>, CollectorError> {
    let mut entries = Vec::new();

    // User autostart directory
    if let Some(home) = std::env::var_os("HOME") {
        let user_autostart = PathBuf::from(home).join(".config/autostart");
        if user_autostart.exists() {
            entries.extend(read_desktop_files(&user_autostart)?);
        }
    }

    // XDG_CONFIG_HOME autostart
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        let autostart = PathBuf::from(config_home).join("autostart");
        if autostart.exists() {
            entries.extend(read_desktop_files(&autostart)?);
        }
    }

    // System autostart directories
    let system_dirs = [
        "/etc/xdg/autostart",
        "/usr/share/gnome/autostart",
        "/usr/share/autostart",
    ];

    for dir in &system_dirs {
        let path = PathBuf::from(dir);
        if path.exists() {
            entries.extend(read_desktop_files(&path)?);
        }
    }

    // Deduplicate by filename (user entries override system entries)
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.filename.clone()));

    Ok(entries)
}

/// Reads .desktop files from a directory.
fn read_desktop_files(dir: &PathBuf) -> Result<Vec<AutostartEntry>, CollectorError> {
    let mut entries = Vec::new();

    let read_dir = fs::read_dir(dir)?;

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Some(parsed) = parse_desktop_file(&path, &content) {
                    entries.push(parsed);
                }
            }
        }
    }

    Ok(entries)
}

/// Parses a .desktop file.
fn parse_desktop_file(path: &PathBuf, content: &str) -> Option<AutostartEntry> {
    let filename = path.file_name()?.to_string_lossy().to_string();

    let mut name = String::new();
    let mut exec = None;
    let mut hidden = false;
    let mut only_show_in = Vec::new();
    let mut not_show_in = Vec::new();

    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }

        if line.starts_with('[') {
            in_desktop_entry = false;
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if let Some(value) = line.strip_prefix("Name=") {
            name = value.to_string();
        } else if let Some(value) = line.strip_prefix("Exec=") {
            exec = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("Hidden=") {
            hidden = value.eq_ignore_ascii_case("true");
        } else if let Some(value) = line.strip_prefix("OnlyShowIn=") {
            only_show_in = value.split(';').filter(|s| !s.is_empty()).map(String::from).collect();
        } else if let Some(value) = line.strip_prefix("NotShowIn=") {
            not_show_in = value.split(';').filter(|s| !s.is_empty()).map(String::from).collect();
        }
    }

    if name.is_empty() {
        name = filename.clone();
    }

    Some(AutostartEntry {
        filename,
        name,
        exec,
        hidden,
        only_show_in,
        not_show_in,
    })
}

/// Collects user systemd timers.
fn collect_user_timers() -> Result<Vec<TimerInfo>, CollectorError> {
    let output = Command::new("systemctl")
        .args(["--user", "list-timers", "--no-legend", "--no-pager", "--all"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("systemctl list-timers failed: {e}")))?;

    if !output.status.success() {
        // User session might not be available
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut timers = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // Format varies, but timer name is typically last
        if parts.len() >= 2 {
            // Try to find the timer unit name (ends with .timer)
            if let Some(timer_name) = parts.iter().find(|p| p.ends_with(".timer")) {
                timers.push(TimerInfo {
                    name: timer_name.to_string(),
                    next_run: None,  // Would need to parse date/time
                    last_run: None,
                    enabled: true,  // Listed timers are typically enabled
                });
            }
        }
    }

    Ok(timers)
}
