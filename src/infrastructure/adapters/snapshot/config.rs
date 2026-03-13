//! Config fingerprint collector.
//!
//! Collects hashes of important configuration files to detect changes.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{ConfigFingerprint, ConfigFingerprints, Snapshot};
use chrono::{TimeZone, Utc};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use tracing::debug;

/// Default configuration files to track.
const DEFAULT_CONFIG_ALLOWLIST: &[&str] = &[
    // System identity
    "/etc/hostname",
    "/etc/machine-id",
    // Authentication
    "/etc/passwd",
    "/etc/group",
    "/etc/shadow",
    "/etc/sudoers",
    "/etc/pam.d/system-auth",
    // Network
    "/etc/resolv.conf",
    "/etc/hosts",
    "/etc/NetworkManager/NetworkManager.conf",
    // Security
    "/etc/selinux/config",
    "/etc/ssh/sshd_config",
    // Package management
    "/etc/dnf/dnf.conf",
    "/etc/yum.conf",
    "/etc/apt/sources.list",
    // System
    "/etc/fstab",
    "/etc/default/grub",
    "/etc/sysctl.conf",
    "/etc/environment",
    // Systemd
    "/etc/systemd/system.conf",
    "/etc/systemd/user.conf",
    "/etc/systemd/journald.conf",
];

/// Collects configuration file fingerprints.
pub struct ConfigFingerprintCollector {
    allowlist: Vec<PathBuf>,
    denylist: Vec<PathBuf>,
}

impl ConfigFingerprintCollector {
    /// Creates a collector with default allowlist.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self {
            allowlist: DEFAULT_CONFIG_ALLOWLIST
                .iter()
                .map(PathBuf::from)
                .collect(),
            denylist: Vec::new(),
        }
    }

    /// Creates a collector with custom allowlist.
    #[must_use]
    pub fn with_allowlist(allowlist: Vec<PathBuf>) -> Self {
        Self {
            allowlist,
            denylist: Vec::new(),
        }
    }

    /// Adds paths to the denylist.
    #[must_use]
    pub fn with_denylist(mut self, denylist: Vec<PathBuf>) -> Self {
        self.denylist = denylist;
        self
    }
}

impl SnapshotCollector for ConfigFingerprintCollector {
    fn name(&self) -> &'static str {
        "config_fingerprints"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self, snapshot: &mut Snapshot, _redact: bool) -> Result<(), CollectorError> {
        let mut fingerprints = BTreeMap::new();

        for path in &self.allowlist {
            // Skip if in denylist
            if self.denylist.iter().any(|d| path.starts_with(d)) {
                continue;
            }

            let fingerprint = collect_file_fingerprint(path);
            fingerprints.insert(path.clone(), fingerprint);
        }

        debug!(count = fingerprints.len(), "Collected config fingerprints");

        snapshot.config_fingerprints = ConfigFingerprints {
            allowlist: self.allowlist.clone(),
            denylist: self.denylist.clone(),
            fingerprints,
        };

        Ok(())
    }
}

/// Collects fingerprint for a single file.
fn collect_file_fingerprint(path: &PathBuf) -> ConfigFingerprint {
    // Check if file exists
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => {
            return ConfigFingerprint {
                hash: String::new(),
                mtime: None,
                size: 0,
                exists: false,
            };
        }
    };

    // Get modification time
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).single())
        .flatten();

    let size = metadata.len();

    // Compute hash
    let hash = match fs::read(path) {
        Ok(content) => {
            let mut hasher = Sha256::new();
            hasher.update(&content);
            hex::encode(hasher.finalize())
        }
        Err(_) => {
            // File exists but can't be read (permissions)
            return ConfigFingerprint {
                hash: "UNREADABLE".to_string(),
                mtime,
                size,
                exists: true,
            };
        }
    };

    ConfigFingerprint {
        hash,
        mtime,
        size,
        exists: true,
    }
}
