//! Network baseline collector.
//!
//! Collects network configuration: gateway, DNS, interfaces.

use super::{CollectorError, SnapshotCollector};
use crate::domain::snapshot::{NetworkBaseline, NetworkInterface, Snapshot};
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;
use tracing::debug;

/// Collects network configuration information.
pub struct NetworkCollector;

impl SnapshotCollector for NetworkCollector {
    fn name(&self) -> &'static str {
        "network"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn collect(&self, snapshot: &mut Snapshot, redact: bool) -> Result<(), CollectorError> {
        let mut network = NetworkBaseline::default();

        // Collect default gateway
        network.default_gateway = collect_default_gateway(redact);

        // Collect DNS servers
        let (dns, search) = collect_dns_config();
        network.dns_servers = if redact {
            dns.iter().map(|s| redact_ip(s)).collect()
        } else {
            dns
        };
        network.search_domains = search;

        // Collect interfaces
        network.interfaces = collect_interfaces(redact)?;

        debug!(
            gateway = ?network.default_gateway,
            dns_count = network.dns_servers.len(),
            if_count = network.interfaces.len(),
            "Collected network baseline"
        );

        snapshot.network = network;
        Ok(())
    }
}

/// Collects the default gateway.
fn collect_default_gateway(redact: bool) -> Option<String> {
    // Try reading from /proc/net/route
    if let Ok(content) = fs::read_to_string("/proc/net/route") {
        for line in content.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                // Destination 00000000 means default route
                if parts[1] == "00000000" {
                    // Gateway is in hex format
                    if let Ok(gw_hex) = u32::from_str_radix(parts[2], 16) {
                        let gw = format!(
                            "{}.{}.{}.{}",
                            gw_hex & 0xFF,
                            (gw_hex >> 8) & 0xFF,
                            (gw_hex >> 16) & 0xFF,
                            (gw_hex >> 24) & 0xFF
                        );
                        return Some(if redact { redact_ip(&gw) } else { gw });
                    }
                }
            }
        }
    }

    // Fallback: use ip route command
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Format: default via 192.168.1.1 dev eth0 ...
        if let Some(line) = stdout.lines().next() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[0] == "default" && parts[1] == "via" {
                let gw = parts[2].to_string();
                return Some(if redact { redact_ip(&gw) } else { gw });
            }
        }
    }

    None
}

/// Collects DNS configuration from resolv.conf.
fn collect_dns_config() -> (Vec<String>, Vec<String>) {
    let mut dns_servers = Vec::new();
    let mut search_domains = Vec::new();

    if let Ok(content) = fs::read_to_string("/etc/resolv.conf") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }

            if let Some(rest) = line.strip_prefix("nameserver") {
                if let Some(server) = rest.split_whitespace().next() {
                    dns_servers.push(server.to_string());
                }
            } else if let Some(rest) = line.strip_prefix("search") {
                search_domains.extend(rest.split_whitespace().map(String::from));
            }
        }
    }

    (dns_servers, search_domains)
}

/// Collects network interfaces.
fn collect_interfaces(redact: bool) -> Result<Vec<NetworkInterface>, CollectorError> {
    let mut interfaces = Vec::new();

    // Use ip command for interface info
    let output = Command::new("ip")
        .args(["-o", "addr", "show"])
        .output()
        .map_err(|e| CollectorError::CommandFailed(format!("ip addr failed: {e}")))?;

    if !output.status.success() {
        return Ok(interfaces);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut current_if: Option<NetworkInterface> = None;

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        // Format: 2: eth0    inet 192.168.1.100/24 ...
        let if_name = parts[1].trim_end_matches(':');

        // Skip loopback
        if if_name == "lo" {
            continue;
        }

        // Check if this is a new interface or continuation
        if current_if.as_ref().map(|i| i.name.as_str()) != Some(if_name) {
            // Save previous interface
            if let Some(iface) = current_if.take() {
                interfaces.push(iface);
            }

            current_if = Some(NetworkInterface {
                name: if_name.to_string(),
                is_up: true, // Assume up if it has addresses
                mac_address: None,
                ip_addresses: Vec::new(),
            });
        }

        // Extract IP address
        if let Some(iface) = &mut current_if {
            if parts.len() >= 4 {
                let addr_type = parts[2]; // inet or inet6
                if addr_type == "inet" || addr_type == "inet6" {
                    let ip = parts[3].split('/').next().unwrap_or(parts[3]);
                    let ip_str = if redact {
                        redact_ip(ip)
                    } else {
                        ip.to_string()
                    };
                    iface.ip_addresses.push(ip_str);
                }
            }
        }
    }

    // Don't forget the last interface
    if let Some(iface) = current_if {
        interfaces.push(iface);
    }

    // Get MAC addresses
    let link_output = Command::new("ip")
        .args(["-o", "link", "show"])
        .output();

    if let Ok(output) = link_output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let if_name = parts[1].trim_end_matches(':');
                    // Find link/ether in the line
                    if let Some(pos) = parts.iter().position(|&p| p == "link/ether") {
                        if parts.len() > pos + 1 {
                            let mac = parts[pos + 1];
                            // Find the interface and set MAC
                            if let Some(iface) = interfaces.iter_mut().find(|i| i.name == if_name) {
                                iface.mac_address = Some(if redact {
                                    redact_mac(mac)
                                } else {
                                    mac.to_string()
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(interfaces)
}

/// Redacts an IP address using a stable hash.
fn redact_ip(ip: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(ip.as_bytes());
    let hash = hasher.finalize();
    format!("IP_{}", hex::encode(&hash[..4]))
}

/// Redacts a MAC address using a stable hash.
fn redact_mac(mac: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(mac.as_bytes());
    let hash = hasher.finalize();
    format!("MAC_{}", hex::encode(&hash[..4]))
}
