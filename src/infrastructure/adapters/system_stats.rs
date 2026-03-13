//! System statistics adapter for reading CPU, memory, disk, and uptime info.
//!
//! Reads from /proc and /sys on Linux systems.

use std::fs;
use tracing::warn;

/// CPU usage statistics.
#[derive(Debug, Clone, Default)]
pub struct CpuStats {
    /// Overall CPU usage percentage (0-100).
    pub usage_percent: f32,
    /// Number of CPU cores.
    pub core_count: usize,
    /// CPU model name.
    pub model_name: String,
}

/// Memory usage statistics.
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    /// Total memory in bytes.
    pub total_bytes: u64,
    /// Used memory in bytes.
    pub used_bytes: u64,
    /// Available memory in bytes.
    pub available_bytes: u64,
    /// Swap total in bytes.
    pub swap_total_bytes: u64,
    /// Swap used in bytes.
    pub swap_used_bytes: u64,
}

impl MemoryStats {
    /// Returns usage percentage (0-100).
    pub fn usage_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f32 / self.total_bytes as f32) * 100.0
    }

    /// Returns swap usage percentage (0-100).
    pub fn swap_usage_percent(&self) -> f32 {
        if self.swap_total_bytes == 0 {
            return 0.0;
        }
        (self.swap_used_bytes as f32 / self.swap_total_bytes as f32) * 100.0
    }

    /// Formats bytes as human-readable.
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.0} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.0} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}

/// Disk usage statistics.
#[derive(Debug, Clone)]
pub struct DiskStats {
    /// Mount point.
    pub mount_point: String,
    /// Filesystem type.
    pub fs_type: String,
    /// Device name.
    pub device: String,
    /// Total space in bytes.
    pub total_bytes: u64,
    /// Used space in bytes.
    pub used_bytes: u64,
    /// Available space in bytes.
    pub available_bytes: u64,
}

impl DiskStats {
    /// Returns usage percentage (0-100).
    pub fn usage_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f32 / self.total_bytes as f32) * 100.0
    }
}

/// System uptime information.
#[derive(Debug, Clone, Default)]
pub struct UptimeInfo {
    /// Uptime in seconds.
    pub uptime_secs: u64,
    /// Idle time in seconds.
    pub idle_secs: u64,
}

impl UptimeInfo {
    /// Formats uptime as human-readable string.
    pub fn format(&self) -> String {
        let days = self.uptime_secs / 86400;
        let hours = (self.uptime_secs % 86400) / 3600;
        let mins = (self.uptime_secs % 3600) / 60;

        if days > 0 {
            format!("{} days, {} hours, {} mins", days, hours, mins)
        } else if hours > 0 {
            format!("{} hours, {} mins", hours, mins)
        } else {
            format!("{} mins", mins)
        }
    }
}

/// Load average information.
#[derive(Debug, Clone, Default)]
pub struct LoadAverage {
    /// 1-minute load average.
    pub one_min: f32,
    /// 5-minute load average.
    pub five_min: f32,
    /// 15-minute load average.
    pub fifteen_min: f32,
}

/// Overall system health summary.
#[derive(Debug, Clone, Default)]
pub struct SystemHealth {
    /// CPU statistics.
    pub cpu: CpuStats,
    /// Memory statistics.
    pub memory: MemoryStats,
    /// Disk statistics.
    pub disks: Vec<DiskStats>,
    /// System uptime.
    pub uptime: UptimeInfo,
    /// Load average.
    pub load: LoadAverage,
    /// System hostname.
    pub hostname: String,
    /// Kernel version string.
    pub kernel_version: String,
    /// OS name and version.
    pub os_name: String,
}

/// Adapter for reading system statistics.
pub struct SystemStatsAdapter;

impl SystemStatsAdapter {
    /// Reads complete system health information.
    pub fn read_system_health() -> SystemHealth {
        SystemHealth {
            cpu: Self::read_cpu_stats(),
            memory: Self::read_memory_stats(),
            disks: Self::read_disk_stats(),
            uptime: Self::read_uptime(),
            load: Self::read_load_average(),
            hostname: Self::read_hostname(),
            kernel_version: Self::read_kernel_version(),
            os_name: Self::read_os_name(),
        }
    }

    /// Reads CPU statistics.
    pub fn read_cpu_stats() -> CpuStats {
        let mut stats = CpuStats::default();

        // Read CPU model name
        if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
            for line in cpuinfo.lines() {
                if line.starts_with("model name") {
                    if let Some(name) = line.split(':').nth(1) {
                        stats.model_name = name.trim().to_string();
                    }
                }
                if line.starts_with("processor") {
                    stats.core_count += 1;
                }
            }
        }

        // Read CPU usage from /proc/stat
        // This is a simplified version - for accurate usage, we'd need to sample twice
        if let Ok(stat) = fs::read_to_string("/proc/stat") {
            if let Some(cpu_line) = stat.lines().next() {
                let parts: Vec<u64> = cpu_line
                    .split_whitespace()
                    .skip(1) // Skip "cpu"
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if parts.len() >= 4 {
                    let user = parts[0];
                    let nice = parts[1];
                    let system = parts[2];
                    let idle = parts[3];
                    let iowait = parts.get(4).copied().unwrap_or(0);

                    let total = user + nice + system + idle + iowait;
                    let busy = user + nice + system;

                    if total > 0 {
                        stats.usage_percent = (busy as f32 / total as f32) * 100.0;
                    }
                }
            }
        }

        stats
    }

    /// Reads memory statistics from /proc/meminfo.
    pub fn read_memory_stats() -> MemoryStats {
        let mut stats = MemoryStats::default();

        let meminfo = match fs::read_to_string("/proc/meminfo") {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "Failed to read /proc/meminfo");
                return stats;
            }
        };

        for line in meminfo.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }

            let key = parts[0].trim_end_matches(':');
            let value: u64 = parts[1].parse().unwrap_or(0);
            // Values in /proc/meminfo are in kB
            let value_bytes = value * 1024;

            match key {
                "MemTotal" => stats.total_bytes = value_bytes,
                "MemAvailable" => stats.available_bytes = value_bytes,
                "SwapTotal" => stats.swap_total_bytes = value_bytes,
                "SwapFree" => {
                    // Swap used = total - free
                    stats.swap_used_bytes = stats.swap_total_bytes.saturating_sub(value_bytes);
                }
                _ => {}
            }
        }

        // Used = Total - Available
        stats.used_bytes = stats.total_bytes.saturating_sub(stats.available_bytes);

        stats
    }

    /// Reads disk statistics using df command (more reliable than statvfs).
    pub fn read_disk_stats() -> Vec<DiskStats> {
        // Use df -T --block-size=1 (not --output which is mutually exclusive with -T)
        let output = match std::process::Command::new("df")
            .args(["-T", "--block-size=1"])
            .output()
        {
            Ok(o) => o,
            Err(e) => {
                warn!(error = %e, "Failed to run df");
                return vec![];
            }
        };

        if !output.status.success() {
            warn!("df command failed");
            return vec![];
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut disks = Vec::new();

        // Skip header line
        // Format: Filesystem Type 1B-blocks Used Available Use% Mounted on
        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 7 {
                continue;
            }

            let device = parts[0];
            let fs_type = parts[1];
            // Mount point is the last field (index 6+)
            let mount_point = parts[6..].join(" ");

            // Skip pseudo-filesystems and virtual mounts
            if fs_type == "tmpfs"
                || fs_type == "devtmpfs"
                || fs_type == "squashfs"
                || fs_type == "overlay"
                || fs_type == "efivarfs"
                || mount_point.starts_with("/snap")
                || mount_point.starts_with("/run")
                || mount_point.starts_with("/dev")
                || mount_point.starts_with("/sys")
                || mount_point.starts_with("/proc")
            {
                continue;
            }

            // Indices: 0=Filesystem, 1=Type, 2=1B-blocks, 3=Used, 4=Available, 5=Use%, 6+=Mounted
            let total: u64 = parts[2].parse().unwrap_or(0);
            let used: u64 = parts[3].parse().unwrap_or(0);
            let avail: u64 = parts[4].parse().unwrap_or(0);

            disks.push(DiskStats {
                mount_point: mount_point.clone(),
                fs_type: fs_type.to_string(),
                device: device.to_string(),
                total_bytes: total,
                used_bytes: used,
                available_bytes: avail,
            });
        }

        disks
    }

    /// Reads system uptime from /proc/uptime.
    pub fn read_uptime() -> UptimeInfo {
        let content = match fs::read_to_string("/proc/uptime") {
            Ok(s) => s,
            Err(_) => return UptimeInfo::default(),
        };

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 2 {
            return UptimeInfo::default();
        }

        let uptime: f64 = parts[0].parse().unwrap_or(0.0);
        let idle: f64 = parts[1].parse().unwrap_or(0.0);

        UptimeInfo {
            uptime_secs: uptime as u64,
            idle_secs: idle as u64,
        }
    }

    /// Reads load average from /proc/loadavg.
    pub fn read_load_average() -> LoadAverage {
        let content = match fs::read_to_string("/proc/loadavg") {
            Ok(s) => s,
            Err(_) => return LoadAverage::default(),
        };

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 3 {
            return LoadAverage::default();
        }

        LoadAverage {
            one_min: parts[0].parse().unwrap_or(0.0),
            five_min: parts[1].parse().unwrap_or(0.0),
            fifteen_min: parts[2].parse().unwrap_or(0.0),
        }
    }

    /// Reads the hostname.
    pub fn read_hostname() -> String {
        fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Reads the kernel version.
    pub fn read_kernel_version() -> String {
        fs::read_to_string("/proc/version")
            .ok()
            .and_then(|s| s.split_whitespace().nth(2).map(String::from))
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Reads the OS name from /etc/os-release.
    pub fn read_os_name() -> String {
        let content = match fs::read_to_string("/etc/os-release") {
            Ok(s) => s,
            Err(_) => return "Linux".to_string(),
        };

        for line in content.lines() {
            if line.starts_with("PRETTY_NAME=") {
                return line
                    .trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string();
            }
        }

        "Linux".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_format_bytes() {
        assert_eq!(MemoryStats::format_bytes(500), "500 B");
        assert_eq!(MemoryStats::format_bytes(1024), "1 KB");
        assert_eq!(MemoryStats::format_bytes(1024 * 1024), "1 MB");
        assert_eq!(MemoryStats::format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn test_uptime_format() {
        let uptime = UptimeInfo {
            uptime_secs: 90061, // 1 day, 1 hour, 1 min
            idle_secs: 0,
        };
        assert_eq!(uptime.format(), "1 days, 1 hours, 1 mins");
    }

    #[test]
    fn test_memory_usage_percent() {
        let mem = MemoryStats {
            total_bytes: 100,
            used_bytes: 50,
            ..Default::default()
        };
        assert!((mem.usage_percent() - 50.0).abs() < 0.01);
    }
}
