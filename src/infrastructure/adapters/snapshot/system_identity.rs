//! System identity collector.
//!
//! Collects hostname, OS info, kernel version, and architecture.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{Snapshot, SystemIdentity};
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;

/// Collects system identity information.
pub struct SystemIdentityCollector;

impl SnapshotCollector for SystemIdentityCollector {
    fn name(&self) -> &'static str {
        "system_identity"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self, snapshot: &mut Snapshot, redact: bool) -> Result<(), CollectorError> {
        let mut identity = SystemIdentity::default();

        // Hostname
        identity.hostname = match fs::read_to_string("/etc/hostname") {
            Ok(h) => {
                let hostname = h.trim().to_string();
                if redact {
                    redact_string(&hostname)
                } else {
                    hostname
                }
            }
            Err(_) => {
                // Fallback to hostname command
                let output = Command::new("hostname").output();
                match output {
                    Ok(o) => {
                        let hostname = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if redact {
                            redact_string(&hostname)
                        } else {
                            hostname
                        }
                    }
                    Err(_) => "unknown".to_string(),
                }
            }
        };

        // OS info from /etc/os-release
        if let Ok(content) = fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(value) = line.strip_prefix("NAME=") {
                    identity.os_name = value.trim_matches('"').to_string();
                } else if let Some(value) = line.strip_prefix("VERSION_ID=") {
                    identity.os_version = value.trim_matches('"').to_string();
                }
            }
        }

        // Kernel version
        if let Ok(uname) = fs::read_to_string("/proc/sys/kernel/osrelease") {
            identity.kernel_version = uname.trim().to_string();
        }

        // Architecture
        let arch_output = Command::new("uname").arg("-m").output();
        if let Ok(output) = arch_output {
            identity.architecture = String::from_utf8_lossy(&output.stdout).trim().to_string();
        }

        snapshot.system_identity = identity;
        Ok(())
    }
}

/// Creates a stable redacted placeholder using SHA256 hash.
fn redact_string(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    format!("REDACTED_{}", hex::encode(&hash[..4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_string_stable() {
        let input = "my-hostname";
        let result1 = redact_string(input);
        let result2 = redact_string(input);
        assert_eq!(result1, result2);
        assert!(result1.starts_with("REDACTED_"));
    }

    #[test]
    fn test_redact_string_different_inputs() {
        let result1 = redact_string("host1");
        let result2 = redact_string("host2");
        assert_ne!(result1, result2);
    }
}
