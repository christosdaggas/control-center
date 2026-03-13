//! Security posture snapshot collector.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::Snapshot;
use crate::infrastructure::adapters::SecurityAdapter;
use tracing::debug;

/// Collects security posture state for snapshots.
pub struct SecurityCollector;

impl SnapshotCollector for SecurityCollector {
    fn name(&self) -> &'static str {
        "security"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self, snapshot: &mut Snapshot, redact: bool) -> Result<(), CollectorError> {
        snapshot.security = SecurityAdapter::collect(redact);

        debug!(
            public_listeners = snapshot
                .security
                .listening_sockets
                .iter()
                .filter(|socket| socket.public)
                .count(),
            admin_accounts = snapshot.security.admin_accounts.len(),
            risky_flatpaks = snapshot.security.flatpak_permissions.len(),
            "Collected security posture"
        );

        Ok(())
    }
}
