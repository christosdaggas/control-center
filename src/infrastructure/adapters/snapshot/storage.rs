//! Storage baseline collector.
//!
//! Collects mounted filesystems and disk usage.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{MountInfo, Snapshot, StorageBaseline};
use std::fs;
use std::process::Command;
use tracing::debug;

/// Collects storage configuration information.
pub struct StorageCollector;

impl SnapshotCollector for StorageCollector {
    fn name(&self) -> &'static str {
        "storage"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self, snapshot: &mut Snapshot, _redact: bool) -> Result<(), CollectorError> {
        let mounts = collect_mounts()?;
        debug!(count = mounts.len(), "Collected mount points");

        snapshot.storage = StorageBaseline { mounts };
        Ok(())
    }
}

/// Collects mount information.
fn collect_mounts() -> Result<Vec<MountInfo>, CollectorError> {
    let mut mounts = Vec::new();

    // Use df command for reliable output
    let output = Command::new("df")
        .args(["--block-size=1", "--output=source,target,fstype,size,used,avail"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("df failed: {e}")))?;

    if !output.status.success() {
        // Fallback: read /proc/mounts
        return collect_mounts_from_proc();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines().skip(1) {
        // Skip header
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 6 {
            let device = parts[0].to_string();
            let mount_point = parts[1].to_string();
            let fs_type = parts[2].to_string();

            // Skip virtual filesystems
            if is_virtual_fs(&fs_type) {
                continue;
            }

            // Skip snap mounts
            if mount_point.starts_with("/snap/") {
                continue;
            }

            let total_bytes = parts[3].parse::<u64>().unwrap_or(0);
            let used_bytes = parts[4].parse::<u64>().unwrap_or(0);
            let available_bytes = parts[5].parse::<u64>().unwrap_or(0);

            let usage_percent = if total_bytes > 0 {
                (used_bytes as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };

            mounts.push(MountInfo {
                mount_point,
                device,
                fs_type,
                total_bytes,
                used_bytes,
                available_bytes,
                usage_percent,
            });
        }
    }

    Ok(mounts)
}

/// Fallback: collect mounts from /proc/mounts.
fn collect_mounts_from_proc() -> Result<Vec<MountInfo>, CollectorError> {
    let mut mounts = Vec::new();

    let content = fs::read_to_string("/proc/mounts")?;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let device = parts[0].to_string();
            let mount_point = parts[1].to_string();
            let fs_type = parts[2].to_string();

            // Skip virtual filesystems
            if is_virtual_fs(&fs_type) {
                continue;
            }

            // Skip snap mounts
            if mount_point.starts_with("/snap/") {
                continue;
            }

            // We don't have usage info from /proc/mounts
            // Try to get it with statvfs
            let (total_bytes, used_bytes, available_bytes, usage_percent) =
                get_mount_usage(&mount_point).unwrap_or((0, 0, 0, 0.0));

            mounts.push(MountInfo {
                mount_point,
                device,
                fs_type,
                total_bytes,
                used_bytes,
                available_bytes,
                usage_percent,
            });
        }
    }

    Ok(mounts)
}

/// Gets usage for a mount point using df command.
fn get_mount_usage(mount_point: &str) -> Option<(u64, u64, u64, f64)> {
    let output = Command::new("df")
        .args(["--block-size=1", "--output=size,used,avail", mount_point])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().nth(1)?; // Skip header
    let parts: Vec<&str> = line.split_whitespace().collect();
    
    if parts.len() >= 3 {
        let total = parts[0].parse::<u64>().ok()?;
        let used = parts[1].parse::<u64>().ok()?;
        let available = parts[2].parse::<u64>().ok()?;
        let usage = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        Some((total, used, available, usage))
    } else {
        None
    }
}

/// Checks if a filesystem type is virtual (should be skipped).
fn is_virtual_fs(fs_type: &str) -> bool {
    matches!(
        fs_type,
        "sysfs"
            | "proc"
            | "devtmpfs"
            | "devpts"
            | "tmpfs"
            | "securityfs"
            | "cgroup"
            | "cgroup2"
            | "pstore"
            | "debugfs"
            | "tracefs"
            | "hugetlbfs"
            | "mqueue"
            | "fusectl"
            | "configfs"
            | "binfmt_misc"
            | "autofs"
            | "efivarfs"
            | "bpf"
            | "nsfs"
            | "ramfs"
            | "rpc_pipefs"
            | "overlay"
            | "squashfs"  // Snap packages
    )
}
