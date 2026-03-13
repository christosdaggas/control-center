//! Security posture adapter.
//!
//! Collects a best-effort security baseline from local system state and
//! derives deterministic findings that can be shown in the UI or stored in
//! snapshots.

use super::{journald::JournaldAdapter, EventAdapter};
use crate::domain::event::{EventType, Severity};
use crate::domain::snapshot::{
    AdminAccount, FirewallBackend, FirewallState, FlatpakPermissions, ListeningSocket,
    MacPolicyState, PolicyMode, SecurityState, SecureBootState, SshState,
};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;

static SS_PROCESS_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"\(\("([^"]+)""#).expect("valid ss process regex"));
static AA_ENFORCE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+)\s+profiles are in enforce mode").expect("valid aa-status regex"));
static AA_COMPLAIN_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\d+)\s+profiles are in complain mode").expect("valid aa-status regex"));

/// Security posture plus derived findings.
#[derive(Debug, Clone)]
pub struct SecurityPosture {
    /// Collected raw security state.
    pub state: SecurityState,
    /// Derived findings.
    pub findings: Vec<SecurityFinding>,
    /// Recent SELinux/AppArmor denial count from journald.
    pub recent_denials: usize,
    /// Collection timestamp.
    pub collected_at: DateTime<Utc>,
}

impl SecurityPosture {
    /// Returns the number of public listeners.
    #[must_use]
    pub fn public_listener_count(&self) -> usize {
        self.state
            .listening_sockets
            .iter()
            .filter(|socket| socket.public)
            .count()
    }

    /// Returns the number of risky Flatpak apps.
    #[must_use]
    pub fn risky_flatpak_count(&self) -> usize {
        self.state.flatpak_permissions.len()
    }

    /// Returns the headline for the summary card.
    #[must_use]
    pub fn headline(&self) -> &'static str {
        if self
            .findings
            .iter()
            .any(|finding| finding.severity >= Severity::Error)
        {
            "Attention Needed"
        } else if self
            .findings
            .iter()
            .any(|finding| finding.severity == Severity::Warning)
        {
            "Review Recommended"
        } else {
            "Good Posture"
        }
    }
}

/// Deterministic security finding.
#[derive(Debug, Clone)]
pub struct SecurityFinding {
    /// Severity level.
    pub severity: Severity,
    /// Short title.
    pub title: String,
    /// One-line summary.
    pub summary: String,
    /// Supporting evidence for the user.
    pub evidence: Vec<String>,
}

/// Security posture collector.
pub struct SecurityAdapter;

impl SecurityAdapter {
    /// Collects raw security state.
    #[must_use]
    pub fn collect(redact: bool) -> SecurityState {
        let listening_sockets = collect_listening_sockets(redact);

        SecurityState {
            firewall: collect_firewall_state(),
            mac_policy: collect_mac_policy_state(),
            secure_boot: collect_secure_boot_state(),
            ssh: collect_ssh_state(&listening_sockets, redact),
            admin_accounts: collect_admin_accounts(redact),
            flatpak_permissions: collect_flatpak_permissions(),
            listening_sockets,
        }
    }

    /// Collects posture and derives findings.
    #[must_use]
    pub fn collect_posture(redact: bool) -> SecurityPosture {
        let state = Self::collect(redact);
        let recent_denials = count_recent_denials();
        let findings = analyze_state(&state, recent_denials);

        debug!(
            findings = findings.len(),
            public_listeners = state
                .listening_sockets
                .iter()
                .filter(|socket| socket.public)
                .count(),
            risky_flatpaks = state.flatpak_permissions.len(),
            "Collected security posture"
        );

        SecurityPosture {
            state,
            findings,
            recent_denials,
            collected_at: Utc::now(),
        }
    }
}

fn collect_firewall_state() -> FirewallState {
    let firewalld_zone = command_stdout("firewall-cmd", &["--get-default-zone"]);
    let firewalld_active = is_service_active("firewalld.service");
    if firewalld_active {
        return FirewallState {
            backend: FirewallBackend::Firewalld,
            active: true,
            summary: firewalld_zone.map(|zone| format!("default zone: {zone}")),
        };
    }

    let ufw_status = command_stdout("ufw", &["status", "verbose"]);
    let ufw_active = ufw_status
        .as_deref()
        .map(|status| status.lines().any(|line| line.contains("Status: active")))
        .unwrap_or(false);
    if ufw_active {
        let summary = ufw_status.as_deref().and_then(parse_ufw_default_policy);
        return FirewallState {
            backend: FirewallBackend::Ufw,
            active: true,
            summary,
        };
    }

    if let Some(ruleset) = command_stdout("nft", &["list", "ruleset"]) {
        if nft_rules_active(&ruleset) {
            return FirewallState {
                backend: FirewallBackend::Nftables,
                active: true,
                summary: Some("ruleset present".to_string()),
            };
        }
    }

    if let Some(rules) = command_stdout("iptables", &["-S"]) {
        if iptables_rules_active(&rules) {
            return FirewallState {
                backend: FirewallBackend::Iptables,
                active: true,
                summary: Some("rules present".to_string()),
            };
        }
    }

    if firewalld_zone.is_some() || command_succeeds("systemctl", &["status", "firewalld.service"]) {
        return FirewallState {
            backend: FirewallBackend::Firewalld,
            active: false,
            summary: Some("service inactive".to_string()),
        };
    }

    if ufw_status.is_some() {
        return FirewallState {
            backend: FirewallBackend::Ufw,
            active: false,
            summary: Some("inactive".to_string()),
        };
    }

    FirewallState {
        backend: FirewallBackend::None,
        active: false,
        summary: Some("no active firewall detected".to_string()),
    }
}

fn collect_mac_policy_state() -> MacPolicyState {
    let selinux = collect_selinux_mode();
    let (apparmor, enforce_profiles, complain_profiles) = collect_apparmor_state();

    MacPolicyState {
        selinux,
        apparmor,
        apparmor_enforce_profiles: enforce_profiles,
        apparmor_complain_profiles: complain_profiles,
    }
}

fn collect_selinux_mode() -> PolicyMode {
    if let Some(mode) = command_stdout("getenforce", &[]) {
        match mode.to_ascii_lowercase().as_str() {
            "enforcing" => return PolicyMode::Enforcing,
            "permissive" => return PolicyMode::Permissive,
            "disabled" => return PolicyMode::Disabled,
            _ => {}
        }
    }

    let enforce_path = Path::new("/sys/fs/selinux/enforce");
    if let Ok(value) = fs::read_to_string(enforce_path) {
        return match value.trim() {
            "1" => PolicyMode::Enforcing,
            "0" => PolicyMode::Permissive,
            _ => PolicyMode::Unknown,
        };
    }

    if Path::new("/etc/selinux/config").exists() {
        PolicyMode::Disabled
    } else {
        PolicyMode::NotInstalled
    }
}

fn collect_apparmor_state() -> (PolicyMode, u32, u32) {
    let enabled_path = Path::new("/sys/module/apparmor/parameters/enabled");
    let enabled = fs::read_to_string(enabled_path)
        .map(|value| value.trim().eq_ignore_ascii_case("Y"))
        .unwrap_or(false);

    if !enabled {
        return if enabled_path.exists() {
            (PolicyMode::Disabled, 0, 0)
        } else {
            (PolicyMode::NotInstalled, 0, 0)
        };
    }

    let aa_status = command_stdout("aa-status", &[]).unwrap_or_default();
    let enforce_profiles = capture_count(&AA_ENFORCE_PATTERN, &aa_status);
    let complain_profiles = capture_count(&AA_COMPLAIN_PATTERN, &aa_status);

    let mode = if complain_profiles > 0 && enforce_profiles == 0 {
        PolicyMode::Complain
    } else {
        PolicyMode::Enforcing
    };

    (mode, enforce_profiles, complain_profiles)
}

fn collect_secure_boot_state() -> SecureBootState {
    let efivars_path = Path::new("/sys/firmware/efi/efivars");
    if !efivars_path.exists() {
        return SecureBootState::Unsupported;
    }

    let Ok(entries) = fs::read_dir(efivars_path) else {
        return SecureBootState::Unknown;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if !file_name.starts_with("SecureBoot-") {
            continue;
        }

        let Ok(bytes) = fs::read(path) else {
            return SecureBootState::Unknown;
        };

        if bytes.len() < 5 {
            return SecureBootState::Unknown;
        }

        return match bytes[4] {
            1 => SecureBootState::Enabled,
            0 => SecureBootState::Disabled,
            _ => SecureBootState::Unknown,
        };
    }

    SecureBootState::Unknown
}

fn collect_listening_sockets(redact: bool) -> Vec<ListeningSocket> {
    let Some(stdout) = command_stdout("ss", &["-lntuH", "-p"]) else {
        return Vec::new();
    };

    let mut sockets = stdout
        .lines()
        .filter_map(|line| parse_listening_socket_line(line, redact))
        .collect::<Vec<_>>();

    sockets.sort();
    sockets.dedup();
    sockets
}

fn parse_listening_socket_line(line: &str, redact: bool) -> Option<ListeningSocket> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 6 {
        return None;
    }

    let protocol = parts[0].to_string();
    let local = parts[4];
    let (bind_address, port) = parse_socket_endpoint(local)?;
    let public = is_public_bind(&bind_address);
    let process = if parts.len() > 6 {
        parse_process_name(&parts[6..].join(" "))
    } else {
        None
    };

    let bind_address = if redact && public {
        redact_value("ADDR", &bind_address)
    } else {
        bind_address
    };

    Some(ListeningSocket {
        protocol,
        bind_address,
        port,
        process,
        public,
    })
}

fn parse_socket_endpoint(spec: &str) -> Option<(String, u16)> {
    if let Some(stripped) = spec.strip_prefix('[') {
        let end = stripped.find(']')?;
        let address = stripped[..end].to_string();
        let port_str = stripped[end + 1..].strip_prefix(':')?;
        let port = port_str.parse().ok()?;
        return Some((strip_scope_id(&address), port));
    }

    let (address, port_str) = spec.rsplit_once(':')?;
    let port = port_str.parse().ok()?;
    Some((strip_scope_id(address), port))
}

fn strip_scope_id(address: &str) -> String {
    address
        .trim_matches('[')
        .trim_matches(']')
        .split('%')
        .next()
        .unwrap_or(address)
        .to_string()
}

fn parse_process_name(process_blob: &str) -> Option<String> {
    SS_PROCESS_PATTERN
        .captures(process_blob)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

fn is_public_bind(address: &str) -> bool {
    !matches!(
        address,
        "127.0.0.1" | "::1" | "localhost" | "127.0.0.53" | "127.0.0.54"
    ) && !address.starts_with("127.")
}

fn collect_ssh_state(listening_sockets: &[ListeningSocket], redact: bool) -> SshState {
    let mut ssh = SshState::default();
    for unit in ["sshd.service", "ssh.service"] {
        let active = is_service_active(unit);
        let enabled = command_stdout("systemctl", &["is-enabled", unit])
            .map(|value| value == "enabled")
            .unwrap_or(false);

        if active || enabled || command_succeeds("systemctl", &["status", unit]) {
            ssh.service_active = active;
            ssh.service_enabled = enabled;
            break;
        }
    }

    let settings = read_sshd_settings();
    ssh.password_authentication = settings
        .get("passwordauthentication")
        .map(|value| value.eq_ignore_ascii_case("yes"));
    ssh.permit_root_login = settings.get("permitrootlogin").cloned();

    let configured_port = settings
        .get("port")
        .and_then(|value| value.parse::<u16>().ok());
    let configured_address = settings.get("listenaddress").cloned();

    let mut ports = BTreeSet::new();
    let mut addresses = BTreeSet::new();

    for socket in listening_sockets {
        let ssh_process = socket
            .process
            .as_deref()
            .map(|name| name.contains("ssh"))
            .unwrap_or(false);
        let ssh_port = configured_port
            .map(|port| socket.port == port)
            .unwrap_or(socket.port == 22);

        if ssh_process || (ssh.service_active && ssh_port) {
            ports.insert(socket.port);
            addresses.insert(socket.bind_address.clone());
        }
    }

    if ports.is_empty() {
        if let Some(port) = configured_port {
            ports.insert(port);
        }
    }
    if addresses.is_empty() {
        if let Some(address) = configured_address {
            addresses.insert(if redact && is_public_bind(&address) {
                redact_value("ADDR", &address)
            } else {
                strip_scope_id(&address)
            });
        }
    }

    ssh.listening_ports = ports.into_iter().collect();
    ssh.listening_addresses = addresses.into_iter().collect();
    ssh
}

fn read_sshd_settings() -> BTreeMap<String, String> {
    let mut files = vec![PathBuf::from("/etc/ssh/sshd_config")];
    if let Ok(entries) = fs::read_dir("/etc/ssh/sshd_config.d") {
        let mut includes = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("conf"))
            .collect::<Vec<_>>();
        includes.sort();
        files.extend(includes);
    }

    let mut settings = BTreeMap::new();
    for path in files {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };

        for raw_line in content.lines() {
            let line = raw_line
                .split('#')
                .next()
                .map(str::trim)
                .unwrap_or_default();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else {
                continue;
            };
            let value = parts.collect::<Vec<_>>().join(" ");
            if value.is_empty() {
                continue;
            }

            settings.insert(key.to_ascii_lowercase(), value);
        }
    }

    settings
}

fn collect_admin_accounts(redact: bool) -> Vec<AdminAccount> {
    let Ok(content) = fs::read_to_string("/etc/group") else {
        return Vec::new();
    };

    let privileged_groups = ["sudo", "wheel", "admin"];
    let mut accounts: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 4 || !privileged_groups.contains(&parts[0]) {
            continue;
        }

        for member in parts[3].split(',').map(str::trim).filter(|value| !value.is_empty()) {
            let username = if redact {
                redact_value("USER", member)
            } else {
                member.to_string()
            };
            accounts
                .entry(username)
                .or_default()
                .insert(parts[0].to_string());
        }
    }

    accounts
        .into_iter()
        .map(|(username, groups)| AdminAccount {
            username,
            groups: groups.into_iter().collect(),
        })
        .collect()
}

fn collect_flatpak_permissions() -> BTreeMap<String, FlatpakPermissions> {
    let Some(stdout) = command_stdout("flatpak", &["list", "--app", "--columns=application"]) else {
        return BTreeMap::new();
    };

    let app_ids = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<BTreeSet<_>>();

    let mut permissions = BTreeMap::new();
    for app_id in app_ids {
        let Some(info) = command_stdout("flatpak", &["info", "--show-permissions", app_id]) else {
            continue;
        };

        let broad_permissions = parse_flatpak_permissions(&info);
        if !broad_permissions.is_empty() {
            permissions.insert(
                app_id.to_string(),
                FlatpakPermissions { broad_permissions },
            );
        }
    }

    permissions
}

fn parse_flatpak_permissions(output: &str) -> Vec<String> {
    let mut grants = BTreeSet::new();
    let mut section = String::new();

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(&['[', ']'][..]).to_string();
            continue;
        }

        if let Some(rest) = line.strip_prefix("shared=") {
            if rest.split(';').any(|item| item == "network") {
                grants.insert("network".to_string());
            }
        } else if let Some(rest) = line.strip_prefix("filesystems=") {
            for item in rest.split(';').map(str::trim).filter(|item| !item.is_empty()) {
                if item.starts_with("host-os") {
                    grants.insert("filesystem=host-os".to_string());
                } else if item.starts_with("host-etc") {
                    grants.insert("filesystem=host-etc".to_string());
                } else if item.starts_with("host") {
                    grants.insert("filesystem=host".to_string());
                } else if item.starts_with("home") {
                    grants.insert("filesystem=home".to_string());
                }
            }
        } else if let Some(rest) = line.strip_prefix("devices=") {
            if rest.split(';').any(|item| item == "all") {
                grants.insert("devices=all".to_string());
            }
        } else if let Some(rest) = line.strip_prefix("features=") {
            if rest.split(';').any(|item| item == "devel") {
                grants.insert("feature=devel".to_string());
            }
        } else if section == "System Bus Policy" && line.ends_with("=talk") {
            let bus_name = line.trim_end_matches("=talk");
            grants.insert(format!("system-bus=talk:{bus_name}"));
        }
    }

    grants.into_iter().collect()
}

fn count_recent_denials() -> usize {
    let adapter = JournaldAdapter::new();
    if !adapter.is_available() {
        return 0;
    }

    adapter
        .read_last_hours(24)
        .map(|events| {
            events
                .iter()
                .filter(|event| event.event_type == EventType::PermissionDenied)
                .count()
        })
        .unwrap_or(0)
}

fn analyze_state(state: &SecurityState, recent_denials: usize) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    let public_sockets = state
        .listening_sockets
        .iter()
        .filter(|socket| socket.public)
        .collect::<Vec<_>>();

    if !public_sockets.is_empty() && !state.firewall.active {
        findings.push(SecurityFinding {
            severity: Severity::Critical,
            title: "Public listeners without active firewall".to_string(),
            summary: "Services are listening on non-loopback interfaces and no active firewall was detected.".to_string(),
            evidence: public_sockets
                .iter()
                .take(5)
                .map(|socket| format_socket(socket))
                .collect(),
        });
    } else if !public_sockets.is_empty() {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "Public listeners detected".to_string(),
            summary: "One or more services are reachable beyond loopback. Review whether each listener should be exposed.".to_string(),
            evidence: public_sockets
                .iter()
                .take(5)
                .map(|socket| format_socket(socket))
                .collect(),
        });
    }

    let ssh_public = state.ssh.listening_addresses.iter().any(|address| {
        !address.starts_with("127.")
            && address != "::1"
            && address != "localhost"
    });
    if ssh_public && state.ssh.password_authentication == Some(true) {
        findings.push(SecurityFinding {
            severity: Severity::Error,
            title: "SSH password authentication exposed".to_string(),
            summary: "SSH is listening on a non-loopback interface and password authentication is enabled.".to_string(),
            evidence: state
                .ssh
                .listening_addresses
                .iter()
                .map(|address| format!("{} on port(s) {}", address, join_ports(&state.ssh.listening_ports)))
                .collect(),
        });
    }
    if ssh_public
        && matches!(
            state
                .ssh
                .permit_root_login
                .as_deref()
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("yes")
        )
    {
        findings.push(SecurityFinding {
            severity: Severity::Error,
            title: "SSH root login enabled".to_string(),
            summary: "SSH is exposed and `PermitRootLogin yes` is configured.".to_string(),
            evidence: vec![format!(
                "Listen addresses: {}",
                state.ssh.listening_addresses.join(", ")
            )],
        });
    }

    if state.mac_policy.selinux == PolicyMode::Permissive {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "SELinux is permissive".to_string(),
            summary: "SELinux is installed but not enforcing access control.".to_string(),
            evidence: Vec::new(),
        });
    }
    if state.mac_policy.selinux == PolicyMode::Disabled
        && state.mac_policy.apparmor != PolicyMode::Enforcing
    {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "No enforcing MAC policy detected".to_string(),
            summary: "Neither SELinux nor AppArmor appears to be actively enforcing.".to_string(),
            evidence: Vec::new(),
        });
    }
    if state.mac_policy.apparmor == PolicyMode::Complain {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "AppArmor profiles are only in complain mode".to_string(),
            summary: "AppArmor is loaded but profiles are not actively blocking denied actions.".to_string(),
            evidence: vec![format!(
                "{} complain profile(s)",
                state.mac_policy.apparmor_complain_profiles
            )],
        });
    }

    if recent_denials > 0 {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "Recent security policy denials".to_string(),
            summary: "Recent SELinux or AppArmor denials were detected in the journal.".to_string(),
            evidence: vec![format!("{recent_denials} denial event(s) in the last 24 hours")],
        });
    }

    if state.secure_boot == SecureBootState::Disabled {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "Secure Boot is disabled".to_string(),
            summary: "UEFI Secure Boot is present but not enabled.".to_string(),
            evidence: Vec::new(),
        });
    }

    if state.admin_accounts.len() > 1 {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "Multiple admin-capable accounts".to_string(),
            summary: "Several users belong to local privileged groups.".to_string(),
            evidence: state
                .admin_accounts
                .iter()
                .map(|account| format!("{} ({})", account.username, account.groups.join(", ")))
                .collect(),
        });
    }

    let risky_flatpaks = state
        .flatpak_permissions
        .iter()
        .filter(|(_, permissions)| {
            permissions.broad_permissions.iter().any(|permission| {
                permission.starts_with("filesystem=host")
                    || permission.starts_with("filesystem=home")
                    || permission == "devices=all"
            })
        })
        .collect::<Vec<_>>();
    if !risky_flatpaks.is_empty() {
        findings.push(SecurityFinding {
            severity: Severity::Warning,
            title: "Broad Flatpak permissions granted".to_string(),
            summary: "Some Flatpak applications have broad filesystem or device access.".to_string(),
            evidence: risky_flatpaks
                .iter()
                .map(|(app_id, permissions)| {
                    format!("{app_id}: {}", permissions.broad_permissions.join(", "))
                })
                .collect(),
        });
    }

    if findings.is_empty() {
        findings.push(SecurityFinding {
            severity: Severity::Info,
            title: "No immediate posture issues detected".to_string(),
            summary: "No high-signal security posture issues were found from the collected local state.".to_string(),
            evidence: Vec::new(),
        });
    }

    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.title.cmp(&b.title))
    });
    findings
}

fn format_socket(socket: &ListeningSocket) -> String {
    match &socket.process {
        Some(process) => format!(
            "{} {}:{} ({process})",
            socket.protocol, socket.bind_address, socket.port
        ),
        None => format!("{} {}:{}", socket.protocol, socket.bind_address, socket.port),
    }
}

fn join_ports(ports: &[u16]) -> String {
    ports
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_service_active(unit: &str) -> bool {
    command_stdout("systemctl", &["is-active", unit])
        .map(|state| state == "active")
        .unwrap_or(false)
}

fn command_stdout(command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn command_succeeds(command: &str, args: &[&str]) -> bool {
    Command::new(command)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn parse_ufw_default_policy(status: &str) -> Option<String> {
    status
        .lines()
        .find(|line| line.trim_start().starts_with("Default:"))
        .map(|line| line.trim().to_string())
}

fn nft_rules_active(ruleset: &str) -> bool {
    ruleset.lines().any(|line| line.trim_start().starts_with("table "))
}

fn iptables_rules_active(rules: &str) -> bool {
    rules.lines().any(|line| {
        line.starts_with("-A ")
            || line
                .strip_prefix("-P ")
                .map(|rest| !rest.ends_with(" ACCEPT"))
                .unwrap_or(false)
    })
}

fn capture_count(pattern: &Regex, haystack: &str) -> u32 {
    pattern
        .captures(haystack)
        .and_then(|captures| captures.get(1))
        .and_then(|value| value.as_str().parse::<u32>().ok())
        .unwrap_or(0)
}

fn redact_value(prefix: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();
    format!("{prefix}_{}", hex::encode(&digest[..4]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_socket_endpoint_ipv4() {
        let parsed = parse_socket_endpoint("0.0.0.0:22");
        assert_eq!(parsed, Some(("0.0.0.0".to_string(), 22)));
    }

    #[test]
    fn test_parse_socket_endpoint_ipv6() {
        let parsed = parse_socket_endpoint("[::1]:631");
        assert_eq!(parsed, Some(("::1".to_string(), 631)));
    }

    #[test]
    fn test_parse_listening_socket_line() {
        let line = r#"tcp LISTEN 0 128 0.0.0.0:22 0.0.0.0:* users:(("sshd",pid=812,fd=3))"#;
        let socket = parse_listening_socket_line(line, false).expect("socket");

        assert_eq!(socket.protocol, "tcp");
        assert_eq!(socket.bind_address, "0.0.0.0");
        assert_eq!(socket.port, 22);
        assert_eq!(socket.process.as_deref(), Some("sshd"));
        assert!(socket.public);
    }

    #[test]
    fn test_parse_flatpak_permissions() {
        let sample = r#"
[Context]
shared=network;ipc;
filesystems=home;host;
devices=all;
features=devel;

[System Bus Policy]
org.freedesktop.NetworkManager=talk
"#;

        let permissions = parse_flatpak_permissions(sample);

        assert!(permissions.contains(&"network".to_string()));
        assert!(permissions.contains(&"filesystem=home".to_string()));
        assert!(permissions.contains(&"filesystem=host".to_string()));
        assert!(permissions.contains(&"devices=all".to_string()));
        assert!(permissions.contains(&"feature=devel".to_string()));
        assert!(permissions.contains(&"system-bus=talk:org.freedesktop.NetworkManager".to_string()));
    }
}
