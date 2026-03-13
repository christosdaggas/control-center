//! Maps process IDs and resource usage to systemd units.
//!
//! This adapter reads /proc/[pid]/cgroup to determine which systemd unit
//! a process belongs to, enabling attribution of resource usage to services.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors from unit mapping.
#[derive(Debug, Error)]
pub enum UnitMapperError {
    /// Failed to read proc file.
    #[error("Failed to read {0}: {1}")]
    ReadError(String, std::io::Error),

    /// Failed to parse cgroup data.
    #[error("Failed to parse cgroup: {0}")]
    ParseError(String),
}

pub type UnitMapperResult<T> = Result<T, UnitMapperError>;

/// A systemd unit with optional slice path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SystemdUnit {
    /// The unit name (e.g., "firefox.service", "user@1000.service").
    pub name: String,
    /// The slice hierarchy (e.g., "user.slice/user-1000.slice").
    pub slice: Option<String>,
    /// Whether this is a user session unit.
    pub is_user: bool,
}

impl SystemdUnit {
    /// Returns a display name suitable for UI.
    #[must_use]
    pub fn display_name(&self) -> String {
        // Strip common suffixes for cleaner display
        let name = self.name.strip_suffix(".service")
            .or_else(|| self.name.strip_suffix(".scope"))
            .or_else(|| self.name.strip_suffix(".slice"))
            .unwrap_or(&self.name);
        
        // Handle user session scopes like "app-gnome-firefox-12345.scope"
        if name.starts_with("app-") {
            if let Some(rest) = name.strip_prefix("app-") {
                // Split by '-' and find the app name
                let parts: Vec<&str> = rest.split('-').collect();
                if parts.len() >= 2 {
                    // app-gnome-firefox-12345 -> firefox
                    // app-flatpak-org.mozilla.firefox-12345 -> org.mozilla.firefox
                    let app_name = parts[1..parts.len()-1].join("-");
                    if !app_name.is_empty() {
                        return app_name;
                    }
                }
            }
        }
        
        name.to_string()
    }
}

/// Resource usage for a process.
#[derive(Debug, Clone, Default)]
pub struct ProcessStats {
    /// Process ID.
    pub pid: u32,
    /// Process name (comm).
    pub name: String,
    /// CPU usage percentage (0-100 * cores).
    pub cpu_percent: f32,
    /// Resident memory in bytes.
    pub rss_bytes: u64,
    /// I/O read bytes (lifetime).
    pub read_bytes: u64,
    /// I/O write bytes (lifetime).
    pub write_bytes: u64,
}

/// Aggregated stats for a systemd unit.
#[derive(Debug, Clone, Default)]
pub struct UnitStats {
    /// The unit.
    pub unit: Option<SystemdUnit>,
    /// Number of processes.
    pub process_count: u32,
    /// Total CPU usage percentage.
    pub cpu_percent: f32,
    /// Total RSS memory in bytes.
    pub rss_bytes: u64,
    /// Total I/O read bytes.
    pub read_bytes: u64,
    /// Total I/O write bytes.
    pub write_bytes: u64,
}

/// Adapter for mapping PIDs to systemd units.
pub struct UnitMapper;

impl UnitMapper {
    /// Gets the systemd unit for a process.
    pub fn unit_for_pid(pid: u32) -> UnitMapperResult<Option<SystemdUnit>> {
        let cgroup_path = format!("/proc/{}/cgroup", pid);
        let content = fs::read_to_string(&cgroup_path)
            .map_err(|e| UnitMapperError::ReadError(cgroup_path.clone(), e))?;
        
        Self::parse_cgroup(&content)
    }

    /// Parses cgroup file content to extract systemd unit.
    fn parse_cgroup(content: &str) -> UnitMapperResult<Option<SystemdUnit>> {
        for line in content.lines() {
            // Format: hierarchy:controller:path
            // e.g., "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-firefox.scope"
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() < 3 {
                continue;
            }

            let path = parts[2];
            if path.is_empty() || path == "/" {
                continue;
            }

            // Parse the cgroup path to find the unit
            return Ok(Self::parse_cgroup_path(path));
        }

        Ok(None)
    }

    /// Parses a cgroup path to extract the systemd unit.
    fn parse_cgroup_path(path: &str) -> Option<SystemdUnit> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }

        // Find the most specific unit (last .service, .scope, or .slice)
        let mut unit_name = None;
        let mut slice_path = Vec::new();
        let mut is_user = false;

        for part in &parts {
            if part.ends_with(".slice") {
                slice_path.push(*part);
                if part.starts_with("user-") || *part == "user.slice" {
                    is_user = true;
                }
            } else if part.ends_with(".service") || part.ends_with(".scope") {
                unit_name = Some((*part).to_string());
                if part.starts_with("user@") || part.starts_with("app-") {
                    is_user = true;
                }
            }
        }

        unit_name.map(|name| SystemdUnit {
            name,
            slice: if slice_path.is_empty() {
                None
            } else {
                Some(slice_path.join("/"))
            },
            is_user,
        })
    }

    /// Gets process stats for a PID.
    pub fn stats_for_pid(pid: u32) -> UnitMapperResult<ProcessStats> {
        let stat_path = format!("/proc/{}/stat", pid);
        let _stat_content = fs::read_to_string(&stat_path)
            .map_err(|e| UnitMapperError::ReadError(stat_path.clone(), e))?;

        let comm_path = format!("/proc/{}/comm", pid);
        let name = fs::read_to_string(&comm_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let statm_path = format!("/proc/{}/statm", pid);
        let rss_bytes = Self::parse_statm(&statm_path).unwrap_or(0);

        let io_path = format!("/proc/{}/io", pid);
        let (read_bytes, write_bytes) = Self::parse_io(&io_path).unwrap_or((0, 0));

        Ok(ProcessStats {
            pid,
            name,
            cpu_percent: 0.0, // Requires delta calculation
            rss_bytes,
            read_bytes,
            write_bytes,
        })
    }

    /// Parses /proc/[pid]/statm for RSS.
    fn parse_statm(path: &str) -> Option<u64> {
        let content = fs::read_to_string(path).ok()?;
        let parts: Vec<&str> = content.split_whitespace().collect();
        // Second field is RSS in pages
        let rss_pages: u64 = parts.get(1)?.parse().ok()?;
        let page_size = 4096u64; // Typical page size
        Some(rss_pages * page_size)
    }

    /// Parses /proc/[pid]/io for read/write bytes.
    fn parse_io(path: &str) -> Option<(u64, u64)> {
        let content = fs::read_to_string(path).ok()?;
        let mut read_bytes = 0u64;
        let mut write_bytes = 0u64;

        for line in content.lines() {
            if let Some(value) = line.strip_prefix("read_bytes: ") {
                read_bytes = value.parse().unwrap_or(0);
            } else if let Some(value) = line.strip_prefix("write_bytes: ") {
                write_bytes = value.parse().unwrap_or(0);
            }
        }

        Some((read_bytes, write_bytes))
    }

    /// Lists all running PIDs.
    pub fn list_pids() -> UnitMapperResult<Vec<u32>> {
        let proc_path = Path::new("/proc");
        let mut pids = Vec::new();

        let entries = fs::read_dir(proc_path)
            .map_err(|e| UnitMapperError::ReadError("/proc".to_string(), e))?;

        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(pid) = name.parse::<u32>() {
                    pids.push(pid);
                }
            }
        }

        Ok(pids)
    }

    /// Gets stats aggregated by systemd unit.
    pub fn stats_by_unit() -> UnitMapperResult<HashMap<Option<SystemdUnit>, UnitStats>> {
        let pids = Self::list_pids()?;
        let mut by_unit: HashMap<Option<SystemdUnit>, UnitStats> = HashMap::new();

        for pid in pids {
            let unit = Self::unit_for_pid(pid).ok().flatten();
            let stats = match Self::stats_for_pid(pid) {
                Ok(s) => s,
                Err(_) => continue, // Process may have exited
            };

            let entry = by_unit.entry(unit.clone()).or_insert_with(|| UnitStats {
                unit: unit.clone(),
                ..Default::default()
            });

            entry.process_count += 1;
            entry.cpu_percent += stats.cpu_percent;
            entry.rss_bytes += stats.rss_bytes;
            entry.read_bytes += stats.read_bytes;
            entry.write_bytes += stats.write_bytes;
        }

        Ok(by_unit)
    }

    /// Gets top units by memory usage.
    pub fn top_by_memory(limit: usize) -> UnitMapperResult<Vec<UnitStats>> {
        let by_unit = Self::stats_by_unit()?;
        let mut units: Vec<_> = by_unit.into_values().collect();
        units.sort_by(|a, b| b.rss_bytes.cmp(&a.rss_bytes));
        units.truncate(limit);
        Ok(units)
    }

    /// Gets top units by I/O.
    pub fn top_by_io(limit: usize) -> UnitMapperResult<Vec<UnitStats>> {
        let by_unit = Self::stats_by_unit()?;
        let mut units: Vec<_> = by_unit.into_values().collect();
        units.sort_by(|a, b| {
            (b.read_bytes + b.write_bytes).cmp(&(a.read_bytes + a.write_bytes))
        });
        units.truncate(limit);
        Ok(units)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cgroup_user_scope() {
        let content = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-firefox-12345.scope\n";
        let result = UnitMapper::parse_cgroup(content).unwrap();
        
        assert!(result.is_some());
        let unit = result.unwrap();
        assert_eq!(unit.name, "app-gnome-firefox-12345.scope");
        assert!(unit.is_user);
        assert!(unit.slice.is_some());
    }

    #[test]
    fn test_parse_cgroup_system_service() {
        let content = "0::/system.slice/docker.service\n";
        let result = UnitMapper::parse_cgroup(content).unwrap();
        
        assert!(result.is_some());
        let unit = result.unwrap();
        assert_eq!(unit.name, "docker.service");
        assert!(!unit.is_user);
    }

    #[test]
    fn test_display_name_app_scope() {
        let unit = SystemdUnit {
            name: "app-gnome-firefox-12345.scope".to_string(),
            slice: None,
            is_user: true,
        };
        assert_eq!(unit.display_name(), "firefox");
    }

    #[test]
    fn test_display_name_service() {
        let unit = SystemdUnit {
            name: "docker.service".to_string(),
            slice: None,
            is_user: false,
        };
        assert_eq!(unit.display_name(), "docker");
    }

    #[test]
    fn test_display_name_flatpak() {
        let unit = SystemdUnit {
            name: "app-flatpak-org.mozilla.firefox-12345.scope".to_string(),
            slice: None,
            is_user: true,
        };
        // Should extract the flatpak app id
        let display = unit.display_name();
        assert!(display.contains("mozilla") || display.contains("firefox"));
    }

    #[test]
    fn test_list_pids() {
        let result = UnitMapper::list_pids();
        assert!(result.is_ok());
        let pids = result.unwrap();
        // Should have at least some processes
        assert!(!pids.is_empty());
        // PID 1 (init) should exist
        assert!(pids.contains(&1));
    }

    #[test]
    fn test_unit_for_self() {
        let pid = std::process::id();
        let result = UnitMapper::unit_for_pid(pid);
        println!("Unit for self (PID {}): {:?}", pid, result);
        // Should at least not error
        assert!(result.is_ok());
    }
}
