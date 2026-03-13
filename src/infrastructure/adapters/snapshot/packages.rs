//! Package state collector.
//!
//! Collects installed packages and versions from RPM, DPKG, or pacman.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{PackageInfo, PackageManager, PackageState, Snapshot};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use tracing::debug;

/// Collects package state information.
pub struct PackageCollector {
    package_manager: PackageManager,
}

impl PackageCollector {
    /// Creates a new package collector, auto-detecting the package manager.
    #[must_use]
    pub fn new() -> Self {
        let package_manager = detect_package_manager();
        debug!(manager = ?package_manager, "Detected package manager");
        Self { package_manager }
    }
}

impl Default for PackageCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotCollector for PackageCollector {
    fn name(&self) -> &'static str {
        "packages"
    }

    fn is_available(&self) -> bool {
        self.package_manager != PackageManager::Unknown
    }

    fn collect(&self, snapshot: &mut Snapshot, _redact: bool) -> Result<(), CollectorError> {
        let packages = match self.package_manager {
            PackageManager::Rpm => collect_rpm_packages()?,
            PackageManager::Dpkg => collect_dpkg_packages()?,
            PackageManager::Pacman => collect_pacman_packages()?,
            PackageManager::Unknown => {
                return Err(CollectorError::NotAvailable(
                    "No supported package manager found".to_string(),
                ))
            }
        };

        snapshot.packages = PackageState {
            package_manager: self.package_manager,
            packages,
        };

        debug!(
            count = snapshot.packages.packages.len(),
            "Collected packages"
        );
        Ok(())
    }
}

/// Detects the system package manager.
fn detect_package_manager() -> PackageManager {
    if Path::new("/usr/bin/rpm").exists() || Path::new("/bin/rpm").exists() {
        PackageManager::Rpm
    } else if Path::new("/usr/bin/dpkg").exists() || Path::new("/usr/bin/dpkg-query").exists() {
        PackageManager::Dpkg
    } else if Path::new("/usr/bin/pacman").exists() {
        PackageManager::Pacman
    } else {
        PackageManager::Unknown
    }
}

/// Collects packages from RPM.
fn collect_rpm_packages() -> Result<BTreeMap<String, PackageInfo>, CollectorError> {
    let output = Command::new("rpm")
        .args(["-qa", "--queryformat", "%{NAME}\t%{VERSION}\t%{RELEASE}\t%{ARCH}\n"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("rpm query failed: {e}")))?;

    if !output.status.success() {
        return Err(CollectorError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            let name = parts[0].to_string();
            let info = PackageInfo {
                version: parts[1].to_string(),
                release: Some(parts[2].to_string()),
                arch: Some(parts[3].to_string()),
                repository: None,
                install_date: None,
            };
            packages.insert(name, info);
        }
    }

    Ok(packages)
}

/// Collects packages from DPKG.
fn collect_dpkg_packages() -> Result<BTreeMap<String, PackageInfo>, CollectorError> {
    let output = Command::new("dpkg-query")
        .args(["-W", "-f", "${Package}\t${Version}\t${Architecture}\n"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("dpkg-query failed: {e}")))?;

    if !output.status.success() {
        return Err(CollectorError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let name = parts[0].to_string();
            let info = PackageInfo {
                version: parts[1].to_string(),
                release: None,
                arch: Some(parts[2].to_string()),
                repository: None,
                install_date: None,
            };
            packages.insert(name, info);
        }
    }

    Ok(packages)
}

/// Collects packages from pacman.
fn collect_pacman_packages() -> Result<BTreeMap<String, PackageInfo>, CollectorError> {
    let output = Command::new("pacman")
        .args(["-Q"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("pacman query failed: {e}")))?;

    if !output.status.success() {
        return Err(CollectorError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages = BTreeMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let info = PackageInfo {
                version: parts[1].to_string(),
                release: None,
                arch: None,
                repository: None,
                install_date: None,
            };
            packages.insert(name, info);
        }
    }

    Ok(packages)
}
