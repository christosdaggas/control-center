//! Snapshot comparison use case.
//!
//! Provides deterministic diff logic for comparing two snapshots
//! with documented impact heuristics.

use crate::domain::diff::{
    impact_rules, ChangeType, DiffCategory, DiffEntry, DiffEvidence, Impact, SnapshotDiff,
};
use crate::domain::snapshot::{ActiveState, Snapshot};
use tracing::debug;

/// Compares two snapshots and returns a structured diff.
///
/// The diff is deterministic: same inputs always produce identical output.
/// All changes are grouped by category and assigned impact levels using
/// documented heuristics.
#[must_use]
pub fn compare_snapshots(base: &Snapshot, current: &Snapshot) -> SnapshotDiff {
    let mut diff = SnapshotDiff::new(base.id, &base.name, &current.name);

    // Compare each category
    diff.packages = compare_packages(base, current);
    diff.flatpaks = compare_flatpaks(base, current);
    diff.systemd = compare_systemd(base, current);
    diff.autostart = compare_autostart(base, current);
    diff.config = compare_config(base, current);
    diff.network = compare_network(base, current);
    diff.storage = compare_storage(base, current);
    diff.security = compare_security(base, current);

    debug!(
        total_changes = diff.total_changes(),
        high_impact = diff.high_impact_count(),
        "Snapshot comparison complete"
    );

    diff
}

/// Compares package state between snapshots.
fn compare_packages(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    // Find added and modified packages
    for (name, current_info) in &current.packages.packages {
        if let Some(base_info) = base.packages.packages.get(name) {
            // Package exists in both - check for version change
            if base_info.version != current_info.version {
                let impact = impact_rules::package_impact(name, ChangeType::Modified);
                let explanation = if impact == Impact::High {
                    format!(
                        "Security-sensitive package '{}' was updated. Verify the update is from a trusted source.",
                        name
                    )
                } else {
                    format!("Package '{}' was updated to a new version.", name)
                };

                entries.push(
                    DiffEntry::new(
                        DiffCategory::Packages,
                        ChangeType::Modified,
                        name,
                        impact,
                        explanation,
                    )
                    .with_before(&base_info.version)
                    .with_after(&current_info.version),
                );
            }
        } else {
            // Package was added
            let impact = impact_rules::package_impact(name, ChangeType::Added);
            entries.push(DiffEntry::new(
                DiffCategory::Packages,
                ChangeType::Added,
                name,
                impact,
                format!("New package '{}' was installed.", name),
            ).with_after(&current_info.version));
        }
    }

    // Find removed packages
    for (name, base_info) in &base.packages.packages {
        if !current.packages.packages.contains_key(name) {
            let impact = impact_rules::package_impact(name, ChangeType::Removed);
            let explanation = if impact == Impact::High {
                format!(
                    "Security-sensitive package '{}' was removed. This may affect system security.",
                    name
                )
            } else {
                format!("Package '{}' was removed.", name)
            };

            entries.push(
                DiffEntry::new(
                    DiffCategory::Packages,
                    ChangeType::Removed,
                    name,
                    impact,
                    explanation,
                )
                .with_before(&base_info.version),
            );
        }
    }

    // Sort for deterministic output
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Compares Flatpak applications between snapshots.
fn compare_flatpaks(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    // Find added and modified Flatpak apps
    for (app_id, current_app) in &current.flatpaks.apps {
        if let Some(base_app) = base.flatpaks.apps.get(app_id) {
            // App exists in both - check for version/branch change
            if base_app.version != current_app.version || base_app.branch != current_app.branch {
                entries.push(
                    DiffEntry::new(
                        DiffCategory::Flatpaks,
                        ChangeType::Modified,
                        &current_app.name,
                        Impact::Low,
                        format!(
                            "Flatpak app '{}' ({}) was updated.",
                            current_app.name, app_id
                        ),
                    )
                    .with_before(format!("{} ({})", base_app.version, base_app.branch))
                    .with_after(format!("{} ({})", current_app.version, current_app.branch)),
                );
            }
        } else {
            // App was added
            entries.push(
                DiffEntry::new(
                    DiffCategory::Flatpaks,
                    ChangeType::Added,
                    &current_app.name,
                    Impact::Low,
                    format!(
                        "Flatpak app '{}' ({}) was installed from {}.",
                        current_app.name, app_id, current_app.origin
                    ),
                )
                .with_after(format!("{} ({})", current_app.version, current_app.branch)),
            );
        }
    }

    // Find removed Flatpak apps
    for (app_id, base_app) in &base.flatpaks.apps {
        if !current.flatpaks.apps.contains_key(app_id) {
            entries.push(
                DiffEntry::new(
                    DiffCategory::Flatpaks,
                    ChangeType::Removed,
                    &base_app.name,
                    Impact::Low,
                    format!("Flatpak app '{}' ({}) was removed.", base_app.name, app_id),
                )
                .with_before(format!("{} ({})", base_app.version, base_app.branch)),
            );
        }
    }

    // Sort for deterministic output
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Compares systemd unit state between snapshots.
fn compare_systemd(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    // Compare system units
    entries.extend(compare_units(
        &base.systemd.system_units,
        &current.systemd.system_units,
        "system",
    ));

    // Compare user units
    entries.extend(compare_units(
        &base.systemd.user_units,
        &current.systemd.user_units,
        "user",
    ));

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Compares a set of units between snapshots.
fn compare_units(
    base: &std::collections::BTreeMap<String, crate::domain::snapshot::UnitState>,
    current: &std::collections::BTreeMap<String, crate::domain::snapshot::UnitState>,
    scope: &str,
) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    for (name, current_state) in current {
        if let Some(base_state) = base.get(name) {
            // Check enablement change
            if base_state.enabled != current_state.enabled {
                let is_failed = current_state.active_state == ActiveState::Failed;
                let impact = impact_rules::systemd_impact(name, ChangeType::Modified, is_failed);

                let explanation = format!(
                    "{} unit '{}' enablement changed from {:?} to {:?}.",
                    scope, name, base_state.enabled, current_state.enabled
                );

                entries.push(
                    DiffEntry::new(
                        DiffCategory::Systemd,
                        ChangeType::Modified,
                        format!("[{}] {}", scope, name),
                        impact,
                        explanation,
                    )
                    .with_before(format!("{:?}", base_state.enabled))
                    .with_after(format!("{:?}", current_state.enabled)),
                );
            }

            // Check active state change (especially failed)
            if base_state.active_state != current_state.active_state {
                let is_now_failed = current_state.active_state == ActiveState::Failed;
                let was_failed = base_state.active_state == ActiveState::Failed;

                if is_now_failed || was_failed {
                    let impact = if is_now_failed {
                        Impact::High
                    } else {
                        Impact::Medium
                    };

                    let explanation = if is_now_failed {
                        format!("{} unit '{}' is now in FAILED state. Check journal for errors.", scope, name)
                    } else {
                        format!("{} unit '{}' recovered from failed state.", scope, name)
                    };

                    entries.push(
                        DiffEntry::new(
                            DiffCategory::Systemd,
                            ChangeType::Modified,
                            format!("[{}] {}", scope, name),
                            impact,
                            explanation,
                        )
                        .with_before(format!("{:?}", base_state.active_state))
                        .with_after(format!("{:?}", current_state.active_state)),
                    );
                }
            }

            // Check for new overrides
            if !base_state.has_overrides && current_state.has_overrides {
                entries.push(DiffEntry::new(
                    DiffCategory::Systemd,
                    ChangeType::Modified,
                    format!("[{}] {}", scope, name),
                    Impact::Medium,
                    format!("{} unit '{}' now has drop-in overrides.", scope, name),
                ));
            }
        } else {
            // New unit
            let is_failed = current_state.active_state == ActiveState::Failed;
            let impact = impact_rules::systemd_impact(name, ChangeType::Added, is_failed);

            entries.push(DiffEntry::new(
                DiffCategory::Systemd,
                ChangeType::Added,
                format!("[{}] {}", scope, name),
                impact,
                format!("New {} unit '{}' appeared.", scope, name),
            ));
        }
    }

    // Find removed units
    for (name, _base_state) in base {
        if !current.contains_key(name) {
            entries.push(DiffEntry::new(
                DiffCategory::Systemd,
                ChangeType::Removed,
                format!("[{}] {}", scope, name),
                Impact::Medium,
                format!("{} unit '{}' was removed.", scope, name),
            ));
        }
    }

    entries
}

/// Compares autostart entries between snapshots.
fn compare_autostart(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    // Compare desktop entries
    let base_entries: std::collections::HashMap<_, _> = base
        .autostart
        .desktop_entries
        .iter()
        .map(|e| (&e.filename, e))
        .collect();

    let current_entries: std::collections::HashMap<_, _> = current
        .autostart
        .desktop_entries
        .iter()
        .map(|e| (&e.filename, e))
        .collect();

    for (filename, current_entry) in &current_entries {
        if let Some(base_entry) = base_entries.get(filename) {
            // Check for hidden state change
            if base_entry.hidden != current_entry.hidden {
                let change = if current_entry.hidden {
                    "disabled"
                } else {
                    "enabled"
                };
                entries.push(DiffEntry::new(
                    DiffCategory::Autostart,
                    ChangeType::Modified,
                    &current_entry.name,
                    Impact::Low,
                    format!("Autostart entry '{}' was {}.", current_entry.name, change),
                ));
            }
        } else {
            entries.push(DiffEntry::new(
                DiffCategory::Autostart,
                ChangeType::Added,
                &current_entry.name,
                Impact::Low,
                format!("New autostart entry '{}' added.", current_entry.name),
            ));
        }
    }

    for (filename, base_entry) in &base_entries {
        if !current_entries.contains_key(filename) {
            entries.push(DiffEntry::new(
                DiffCategory::Autostart,
                ChangeType::Removed,
                &base_entry.name,
                Impact::Low,
                format!("Autostart entry '{}' was removed.", base_entry.name),
            ));
        }
    }

    // Compare user timers
    let base_timers: std::collections::HashSet<_> =
        base.autostart.user_timers.iter().map(|t| &t.name).collect();
    let current_timers: std::collections::HashSet<_> = current
        .autostart
        .user_timers
        .iter()
        .map(|t| &t.name)
        .collect();

    for timer in current_timers.difference(&base_timers) {
        entries.push(DiffEntry::new(
            DiffCategory::Autostart,
            ChangeType::Added,
            *timer,
            Impact::Medium,
            format!("New user timer '{}' was added.", timer),
        ));
    }

    for timer in base_timers.difference(&current_timers) {
        entries.push(DiffEntry::new(
            DiffCategory::Autostart,
            ChangeType::Removed,
            *timer,
            Impact::Medium,
            format!("User timer '{}' was removed.", timer),
        ));
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Compares config fingerprints between snapshots.
fn compare_config(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    for (path, current_fp) in &current.config_fingerprints.fingerprints {
        let path_str = path.to_string_lossy();

        if let Some(base_fp) = base.config_fingerprints.fingerprints.get(path) {
            // Check for changes
            if base_fp.hash != current_fp.hash {
                let impact = impact_rules::config_impact(&path_str);

                let change_desc = if !base_fp.exists && current_fp.exists {
                    "was created"
                } else if base_fp.exists && !current_fp.exists {
                    "was deleted"
                } else {
                    "was modified"
                };

                let explanation = if impact == Impact::High {
                    format!(
                        "Critical config file {} {}. This may affect system security or stability.",
                        path_str, change_desc
                    )
                } else {
                    format!("Config file {} {}.", path_str, change_desc)
                };

                let mut entry = DiffEntry::new(
                    DiffCategory::Config,
                    if !current_fp.exists {
                        ChangeType::Removed
                    } else {
                        ChangeType::Modified
                    },
                    path_str.to_string(),
                    impact,
                    explanation,
                );

                // Add size info as evidence
                if base_fp.exists && current_fp.exists {
                    entry = entry.with_evidence(DiffEvidence::new(
                        "File comparison",
                        format!(
                            "Size: {} → {} bytes\nHash: {} → {}",
                            base_fp.size,
                            current_fp.size,
                            &base_fp.hash[..16.min(base_fp.hash.len())],
                            &current_fp.hash[..16.min(current_fp.hash.len())]
                        ),
                    ).with_file_path(path.clone()));
                }

                entries.push(entry);
            }
        } else if current_fp.exists {
            // New file being tracked
            entries.push(DiffEntry::new(
                DiffCategory::Config,
                ChangeType::Added,
                path_str.to_string(),
                impact_rules::config_impact(&path_str),
                format!("New config file {} appeared.", path_str),
            ));
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

/// Compares network configuration between snapshots.
fn compare_network(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    // Compare default gateway
    if base.network.default_gateway != current.network.default_gateway {
        let impact = impact_rules::network_impact(true, false);
        entries.push(
            DiffEntry::new(
                DiffCategory::Network,
                ChangeType::Modified,
                "Default Gateway",
                impact,
                "Default gateway changed. This affects all outbound network traffic.",
            )
            .with_before(base.network.default_gateway.as_deref().unwrap_or("none"))
            .with_after(current.network.default_gateway.as_deref().unwrap_or("none")),
        );
    }

    // Compare DNS servers
    let base_dns: std::collections::HashSet<_> = base.network.dns_servers.iter().collect();
    let current_dns: std::collections::HashSet<_> = current.network.dns_servers.iter().collect();

    if base_dns != current_dns {
        let impact = impact_rules::network_impact(false, true);
        entries.push(
            DiffEntry::new(
                DiffCategory::Network,
                ChangeType::Modified,
                "DNS Servers",
                impact,
                "DNS server configuration changed. This affects name resolution.",
            )
            .with_before(base.network.dns_servers.join(", "))
            .with_after(current.network.dns_servers.join(", ")),
        );
    }

    // Compare interfaces
    let base_ifs: std::collections::HashMap<_, _> = base
        .network
        .interfaces
        .iter()
        .map(|i| (&i.name, i))
        .collect();

    let current_ifs: std::collections::HashMap<_, _> = current
        .network
        .interfaces
        .iter()
        .map(|i| (&i.name, i))
        .collect();

    for (name, current_if) in &current_ifs {
        if let Some(base_if) = base_ifs.get(name) {
            if base_if.is_up != current_if.is_up {
                entries.push(DiffEntry::new(
                    DiffCategory::Network,
                    ChangeType::Modified,
                    format!("Interface {}", name),
                    Impact::Medium,
                    format!(
                        "Interface '{}' is now {}.",
                        name,
                        if current_if.is_up { "UP" } else { "DOWN" }
                    ),
                ));
            }
        } else {
            entries.push(DiffEntry::new(
                DiffCategory::Network,
                ChangeType::Added,
                format!("Interface {}", name),
                Impact::Low,
                format!("New network interface '{}' appeared.", name),
            ));
        }
    }

    for name in base_ifs.keys() {
        if !current_ifs.contains_key(name) {
            entries.push(DiffEntry::new(
                DiffCategory::Network,
                ChangeType::Removed,
                format!("Interface {}", name),
                Impact::Medium,
                format!("Network interface '{}' is no longer present.", name),
            ));
        }
    }

    entries
}

/// Compares storage configuration between snapshots.
fn compare_storage(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    let base_mounts: std::collections::HashMap<_, _> = base
        .storage
        .mounts
        .iter()
        .map(|m| (&m.mount_point, m))
        .collect();

    let current_mounts: std::collections::HashMap<_, _> = current
        .storage
        .mounts
        .iter()
        .map(|m| (&m.mount_point, m))
        .collect();

    for (mount_point, current_mount) in &current_mounts {
        if let Some(base_mount) = base_mounts.get(mount_point) {
            // Check usage change
            let usage_delta = current_mount.usage_percent - base_mount.usage_percent;

            if usage_delta.abs() > 5.0 {
                let impact = impact_rules::storage_impact(mount_point, usage_delta);

                let explanation = if usage_delta > 0.0 {
                    format!(
                        "Disk usage on '{}' increased by {:.1}%. Currently at {:.1}%.",
                        mount_point, usage_delta, current_mount.usage_percent
                    )
                } else {
                    format!(
                        "Disk usage on '{}' decreased by {:.1}%. Currently at {:.1}%.",
                        mount_point,
                        usage_delta.abs(),
                        current_mount.usage_percent
                    )
                };

                entries.push(
                    DiffEntry::new(
                        DiffCategory::Storage,
                        ChangeType::Modified,
                        *mount_point,
                        impact,
                        explanation,
                    )
                    .with_before(format!("{:.1}%", base_mount.usage_percent))
                    .with_after(format!("{:.1}%", current_mount.usage_percent)),
                );
            }

            // Check device change (unusual but can happen)
            if base_mount.device != current_mount.device {
                entries.push(DiffEntry::new(
                    DiffCategory::Storage,
                    ChangeType::Modified,
                    *mount_point,
                    Impact::High,
                    format!(
                        "Mount point '{}' is now served by a different device.",
                        mount_point
                    ),
                ).with_before(&base_mount.device).with_after(&current_mount.device));
            }
        } else {
            entries.push(DiffEntry::new(
                DiffCategory::Storage,
                ChangeType::Added,
                *mount_point,
                Impact::Medium,
                format!("New mount point '{}' appeared.", mount_point),
            ));
        }
    }

    for mount_point in base_mounts.keys() {
        if !current_mounts.contains_key(mount_point) {
            entries.push(DiffEntry::new(
                DiffCategory::Storage,
                ChangeType::Removed,
                *mount_point,
                Impact::Medium,
                format!("Mount point '{}' is no longer present.", mount_point),
            ));
        }
    }

    entries
}

/// Compares security posture between snapshots.
fn compare_security(base: &Snapshot, current: &Snapshot) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    if base.security.firewall != current.security.firewall {
        let impact = if current.security.firewall.active {
            Impact::Medium
        } else {
            Impact::High
        };

        entries.push(
            DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Modified,
                "Firewall",
                impact,
                "Firewall backend or active status changed.",
            )
            .with_before(format_firewall(&base.security.firewall))
            .with_after(format_firewall(&current.security.firewall)),
        );
    }

    if base.security.mac_policy.selinux != current.security.mac_policy.selinux {
        let impact = match current.security.mac_policy.selinux {
            crate::domain::snapshot::PolicyMode::Disabled
            | crate::domain::snapshot::PolicyMode::Permissive => Impact::High,
            _ => Impact::Medium,
        };

        entries.push(
            DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Modified,
                "SELinux",
                impact,
                "SELinux enforcement mode changed.",
            )
            .with_before(base.security.mac_policy.selinux.label())
            .with_after(current.security.mac_policy.selinux.label()),
        );
    }

    if base.security.mac_policy.apparmor != current.security.mac_policy.apparmor {
        let impact = match current.security.mac_policy.apparmor {
            crate::domain::snapshot::PolicyMode::Disabled
            | crate::domain::snapshot::PolicyMode::Complain => Impact::High,
            _ => Impact::Medium,
        };

        entries.push(
            DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Modified,
                "AppArmor",
                impact,
                "AppArmor enforcement mode changed.",
            )
            .with_before(base.security.mac_policy.apparmor.label())
            .with_after(current.security.mac_policy.apparmor.label()),
        );
    }

    if base.security.secure_boot != current.security.secure_boot {
        let impact = if current.security.secure_boot
            == crate::domain::snapshot::SecureBootState::Disabled
        {
            Impact::High
        } else {
            Impact::Medium
        };

        entries.push(
            DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Modified,
                "Secure Boot",
                impact,
                "Secure Boot state changed.",
            )
            .with_before(base.security.secure_boot.label())
            .with_after(current.security.secure_boot.label()),
        );
    }

    let base_public: std::collections::BTreeMap<_, _> = base
        .security
        .listening_sockets
        .iter()
        .filter(|socket| socket.public)
        .map(|socket| (socket_key(socket), socket))
        .collect();
    let current_public: std::collections::BTreeMap<_, _> = current
        .security
        .listening_sockets
        .iter()
        .filter(|socket| socket.public)
        .map(|socket| (socket_key(socket), socket))
        .collect();

    for (key, socket) in &current_public {
        if !base_public.contains_key(key) {
            entries.push(DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Added,
                format!("Public listener {}:{}", socket.bind_address, socket.port),
                impact_rules::security_socket_impact(socket.port, socket.public),
                format!(
                    "New public listener detected on {}:{}.",
                    socket.bind_address, socket.port
                ),
            ));
        }
    }

    for (key, socket) in &base_public {
        if !current_public.contains_key(key) {
            entries.push(DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Removed,
                format!("Public listener {}:{}", socket.bind_address, socket.port),
                Impact::Medium,
                format!(
                    "Public listener on {}:{} is no longer present.",
                    socket.bind_address, socket.port
                ),
            ));
        }
    }

    if base.security.ssh != current.security.ssh {
        let impact = if current.security.ssh.password_authentication == Some(true)
            || current
                .security
                .ssh
                .permit_root_login
                .as_deref()
                .map(str::to_ascii_lowercase)
                .as_deref()
                == Some("yes")
        {
            Impact::High
        } else {
            Impact::Medium
        };

        entries.push(
            DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Modified,
                "SSH Exposure",
                impact,
                "SSH daemon exposure or authentication settings changed.",
            )
            .with_before(format_ssh(&base.security.ssh))
            .with_after(format_ssh(&current.security.ssh)),
        );
    }

    let base_admins: std::collections::BTreeMap<_, _> = base
        .security
        .admin_accounts
        .iter()
        .map(|account| (&account.username, account))
        .collect();
    let current_admins: std::collections::BTreeMap<_, _> = current
        .security
        .admin_accounts
        .iter()
        .map(|account| (&account.username, account))
        .collect();

    for (username, account) in &current_admins {
        if let Some(base_account) = base_admins.get(username) {
            if base_account.groups != account.groups {
                entries.push(
                    DiffEntry::new(
                        DiffCategory::Security,
                        ChangeType::Modified,
                        format!("Admin account {}", username),
                        Impact::High,
                        "Privileged group membership changed.",
                    )
                    .with_before(base_account.groups.join(", "))
                    .with_after(account.groups.join(", ")),
                );
            }
        } else {
            entries.push(DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Added,
                format!("Admin account {}", username),
                Impact::High,
                format!(
                    "User '{}' now belongs to privileged groups: {}.",
                    username,
                    account.groups.join(", ")
                ),
            ));
        }
    }

    for username in base_admins.keys() {
        if !current_admins.contains_key(username) {
            entries.push(DiffEntry::new(
                DiffCategory::Security,
                ChangeType::Removed,
                format!("Admin account {}", username),
                Impact::Medium,
                format!("User '{}' no longer appears in privileged groups.", username),
            ));
        }
    }

    for (app_id, current_permissions) in &current.security.flatpak_permissions {
        if let Some(base_permissions) = base.security.flatpak_permissions.get(app_id) {
            if base_permissions.broad_permissions != current_permissions.broad_permissions {
                let impact = permission_set_impact(&current_permissions.broad_permissions);
                entries.push(
                    DiffEntry::new(
                        DiffCategory::Security,
                        ChangeType::Modified,
                        format!("Flatpak {}", app_id),
                        impact,
                        "Broad Flatpak permissions changed.",
                    )
                    .with_before(base_permissions.broad_permissions.join(", "))
                    .with_after(current_permissions.broad_permissions.join(", ")),
                );
            }
        } else {
            let impact = permission_set_impact(&current_permissions.broad_permissions);
            entries.push(
                DiffEntry::new(
                    DiffCategory::Security,
                    ChangeType::Added,
                    format!("Flatpak {}", app_id),
                    impact,
                    "Broad Flatpak permissions were detected for this app.",
                )
                .with_after(current_permissions.broad_permissions.join(", ")),
            );
        }
    }

    for (app_id, base_permissions) in &base.security.flatpak_permissions {
        if !current.security.flatpak_permissions.contains_key(app_id) {
            entries.push(
                DiffEntry::new(
                    DiffCategory::Security,
                    ChangeType::Removed,
                    format!("Flatpak {}", app_id),
                    Impact::Low,
                    "Broad Flatpak permissions are no longer present for this app.",
                )
                .with_before(base_permissions.broad_permissions.join(", ")),
            );
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn format_firewall(firewall: &crate::domain::snapshot::FirewallState) -> String {
    let mut parts = vec![firewall.backend.label().to_string()];
    parts.push(if firewall.active {
        "active".to_string()
    } else {
        "inactive".to_string()
    });

    if let Some(summary) = &firewall.summary {
        parts.push(summary.clone());
    }

    parts.join(" • ")
}

fn socket_key(socket: &crate::domain::snapshot::ListeningSocket) -> String {
    format!(
        "{}:{}:{}:{}",
        socket.protocol,
        socket.bind_address,
        socket.port,
        socket.process.as_deref().unwrap_or("")
    )
}

fn format_ssh(ssh: &crate::domain::snapshot::SshState) -> String {
    let ports = if ssh.listening_ports.is_empty() {
        "none".to_string()
    } else {
        ssh.listening_ports
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };

    let addresses = if ssh.listening_addresses.is_empty() {
        "none".to_string()
    } else {
        ssh.listening_addresses.join(", ")
    };

    format!(
        "active={} enabled={} password_auth={} root_login={} ports={} addresses={}",
        ssh.service_active,
        ssh.service_enabled,
        ssh.password_authentication
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        ssh.permit_root_login.as_deref().unwrap_or("unknown"),
        ports,
        addresses
    )
}

fn permission_set_impact(permissions: &[String]) -> Impact {
    permissions
        .iter()
        .map(|permission| impact_rules::security_permission_impact(permission))
        .max()
        .unwrap_or(Impact::Low)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::snapshot::{
        AdminAccount, FirewallBackend, FirewallState, FlatpakPermissions, ListeningSocket,
        PackageInfo, PackageManager, PackageState,
    };

    #[test]
    fn test_compare_empty_snapshots() {
        let base = Snapshot::new("Base");
        let current = Snapshot::new("Current");

        let diff = compare_snapshots(&base, &current);

        assert_eq!(diff.total_changes(), 0);
    }

    #[test]
    fn test_compare_package_added() {
        let base = Snapshot::new("Base");
        let mut current = Snapshot::new("Current");

        current.packages = PackageState {
            package_manager: PackageManager::Rpm,
            packages: [("vim".to_string(), PackageInfo {
                version: "9.0".to_string(),
                release: None,
                arch: None,
                repository: None,
                install_date: None,
            })].into_iter().collect(),
        };

        let diff = compare_snapshots(&base, &current);

        assert_eq!(diff.packages.len(), 1);
        assert_eq!(diff.packages[0].change_type, ChangeType::Added);
        assert_eq!(diff.packages[0].name, "vim");
    }

    #[test]
    fn test_compare_package_modified() {
        let mut base = Snapshot::new("Base");
        let mut current = Snapshot::new("Current");

        base.packages = PackageState {
            package_manager: PackageManager::Rpm,
            packages: [("openssl".to_string(), PackageInfo {
                version: "1.1.1".to_string(),
                release: None,
                arch: None,
                repository: None,
                install_date: None,
            })].into_iter().collect(),
        };

        current.packages = PackageState {
            package_manager: PackageManager::Rpm,
            packages: [("openssl".to_string(), PackageInfo {
                version: "3.0.0".to_string(),
                release: None,
                arch: None,
                repository: None,
                install_date: None,
            })].into_iter().collect(),
        };

        let diff = compare_snapshots(&base, &current);

        assert_eq!(diff.packages.len(), 1);
        assert_eq!(diff.packages[0].change_type, ChangeType::Modified);
        assert_eq!(diff.packages[0].impact, Impact::High); // Security package
    }

    #[test]
    fn test_deterministic_output() {
        let base = Snapshot::new("Base");
        let current = Snapshot::new("Current");

        let diff1 = compare_snapshots(&base, &current);
        let diff2 = compare_snapshots(&base, &current);

        // Same inputs should produce identical outputs
        assert_eq!(
            serde_json::to_string(&diff1).unwrap(),
            serde_json::to_string(&diff2).unwrap()
        );
    }

    #[test]
    fn test_compare_security_changes() {
        let mut base = Snapshot::new("Base");
        let mut current = Snapshot::new("Current");

        base.security.firewall = FirewallState {
            backend: FirewallBackend::Firewalld,
            active: true,
            summary: Some("default zone: public".to_string()),
        };
        current.security.firewall = FirewallState {
            backend: FirewallBackend::None,
            active: false,
            summary: Some("no active firewall detected".to_string()),
        };
        current.security.listening_sockets.push(ListeningSocket {
            protocol: "tcp".to_string(),
            bind_address: "0.0.0.0".to_string(),
            port: 22,
            process: Some("sshd".to_string()),
            public: true,
        });
        current.security.admin_accounts.push(AdminAccount {
            username: "alice".to_string(),
            groups: vec!["wheel".to_string()],
        });
        current.security.flatpak_permissions.insert(
            "com.example.App".to_string(),
            FlatpakPermissions {
                broad_permissions: vec!["filesystem=host".to_string()],
            },
        );

        let diff = compare_snapshots(&base, &current);

        assert!(!diff.security.is_empty());
        assert!(diff
            .security
            .iter()
            .any(|entry| entry.name == "Firewall" && entry.impact == Impact::High));
        assert!(diff
            .security
            .iter()
            .any(|entry| entry.name == "Admin account alice"));
    }
}
