//! Procfs adapters for reading /proc/stat, /proc/meminfo, /proc/vmstat, /proc/diskstats.
//!
//! These provide detailed system metrics beyond PSI.

use crate::domain::pressure::{CpuMetrics, IoMetrics, MemoryMetrics};
use std::collections::HashMap;
use std::fs;
use thiserror::Error;


/// Errors from procfs reading.
#[derive(Debug, Error)]
pub enum ProcfsError {
    /// Failed to read file.
    #[error("Failed to read {0}: {1}")]
    ReadError(String, std::io::Error),

    /// Failed to parse data.
    #[error("Failed to parse {0}: {1}")]
    ParseError(String, String),
}

pub type ProcfsResult<T> = Result<T, ProcfsError>;

/// Raw CPU stats from /proc/stat.
#[derive(Debug, Clone, Default)]
pub struct RawCpuStats {
    /// Time spent in user mode.
    pub user: u64,
    /// Time spent in user mode with low priority (nice).
    pub nice: u64,
    /// Time spent in system mode.
    pub system: u64,
    /// Time spent idle.
    pub idle: u64,
    /// Time waiting for I/O completion.
    pub iowait: u64,
    /// Time servicing hardware interrupts.
    pub irq: u64,
    /// Time servicing software interrupts.
    pub softirq: u64,
    /// Time stolen by other operating systems in virtualized environment.
    pub steal: u64,
    /// Time spent running a virtual CPU for guest OS.
    pub guest: u64,
    /// Time spent running a niced guest.
    pub guest_nice: u64,
}

impl RawCpuStats {
    /// Total CPU time.
    pub fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait
            + self.irq + self.softirq + self.steal
    }

    /// Idle CPU time (idle + iowait).
    pub fn idle_total(&self) -> u64 {
        self.idle + self.iowait
    }

    /// Calculates usage percentage from delta between two samples.
    pub fn usage_percent(prev: &Self, curr: &Self) -> f32 {
        let total_delta = curr.total().saturating_sub(prev.total());
        let idle_delta = curr.idle_total().saturating_sub(prev.idle_total());
        
        if total_delta == 0 {
            return 0.0;
        }
        
        let active_delta = total_delta.saturating_sub(idle_delta);
        (active_delta as f32 / total_delta as f32) * 100.0
    }

    /// Calculates iowait percentage from delta.
    pub fn iowait_percent(prev: &Self, curr: &Self) -> f32 {
        let total_delta = curr.total().saturating_sub(prev.total());
        let iowait_delta = curr.iowait.saturating_sub(prev.iowait);
        
        if total_delta == 0 {
            return 0.0;
        }
        
        (iowait_delta as f32 / total_delta as f32) * 100.0
    }
}

/// Adapter for reading /proc/stat.
pub struct ProcStatAdapter;

impl ProcStatAdapter {
    const PATH: &'static str = "/proc/stat";

    /// Reads current raw CPU stats.
    pub fn read() -> ProcfsResult<RawCpuStats> {
        let content = fs::read_to_string(Self::PATH)
            .map_err(|e| ProcfsError::ReadError(Self::PATH.to_string(), e))?;
        Self::parse(&content)
    }

    /// Parses /proc/stat content.
    fn parse(content: &str) -> ProcfsResult<RawCpuStats> {
        for line in content.lines() {
            if line.starts_with("cpu ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 5 {
                    return Err(ProcfsError::ParseError(
                        "/proc/stat".to_string(),
                        "Not enough CPU fields".to_string(),
                    ));
                }

                let parse_u64 = |idx: usize| -> u64 {
                    parts.get(idx).and_then(|s| s.parse().ok()).unwrap_or(0)
                };

                return Ok(RawCpuStats {
                    user: parse_u64(1),
                    nice: parse_u64(2),
                    system: parse_u64(3),
                    idle: parse_u64(4),
                    iowait: parse_u64(5),
                    irq: parse_u64(6),
                    softirq: parse_u64(7),
                    steal: parse_u64(8),
                    guest: parse_u64(9),
                    guest_nice: parse_u64(10),
                });
            }
        }

        Err(ProcfsError::ParseError(
            "/proc/stat".to_string(),
            "No cpu line found".to_string(),
        ))
    }

    /// Converts raw stats to CpuMetrics using previous sample.
    pub fn to_metrics(prev: &RawCpuStats, curr: &RawCpuStats) -> CpuMetrics {
        let load_avg = Self::read_load_avg().unwrap_or((0.0, 0.0, 0.0));
        CpuMetrics {
            utilization: RawCpuStats::usage_percent(prev, curr),
            iowait: RawCpuStats::iowait_percent(prev, curr),
            system: Self::system_percent(prev, curr),
            user: Self::user_percent(prev, curr),
            load_1m: load_avg.0,
            load_5m: load_avg.1,
            runnable: Self::read_runnable_tasks().unwrap_or(0),
        }
    }

    /// Calculates system CPU percentage from delta.
    fn system_percent(prev: &RawCpuStats, curr: &RawCpuStats) -> f32 {
        let total_delta = curr.total().saturating_sub(prev.total());
        let system_delta = curr.system.saturating_sub(prev.system);
        
        if total_delta == 0 {
            return 0.0;
        }
        
        (system_delta as f32 / total_delta as f32) * 100.0
    }

    /// Calculates user CPU percentage from delta.
    fn user_percent(prev: &RawCpuStats, curr: &RawCpuStats) -> f32 {
        let total_delta = curr.total().saturating_sub(prev.total());
        let user_delta = (curr.user + curr.nice).saturating_sub(prev.user + prev.nice);
        
        if total_delta == 0 {
            return 0.0;
        }
        
        (user_delta as f32 / total_delta as f32) * 100.0
    }

    /// Reads load averages from /proc/loadavg.
    pub fn read_load_avg() -> ProcfsResult<(f32, f32, f32)> {
        let content = fs::read_to_string("/proc/loadavg")
            .map_err(|e| ProcfsError::ReadError("/proc/loadavg".to_string(), e))?;
        
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 3 {
            return Err(ProcfsError::ParseError(
                "/proc/loadavg".to_string(),
                "Not enough fields".to_string(),
            ));
        }

        let parse = |s: &str| -> f32 { s.parse().unwrap_or(0.0) };
        Ok((parse(parts[0]), parse(parts[1]), parse(parts[2])))
    }

    /// Reads runnable task count from /proc/loadavg.
    pub fn read_runnable_tasks() -> ProcfsResult<u32> {
        let content = fs::read_to_string("/proc/loadavg")
            .map_err(|e| ProcfsError::ReadError("/proc/loadavg".to_string(), e))?;
        
        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 4 {
            return Ok(0);
        }

        // Format: "runnable/total"
        if let Some((runnable, _)) = parts[3].split_once('/') {
            return Ok(runnable.parse().unwrap_or(0));
        }

        Ok(0)
    }

    /// Gets the number of CPU cores.
    pub fn cpu_core_count() -> u32 {
        fs::read_to_string("/proc/cpuinfo")
            .ok()
            .map(|content| {
                content
                    .lines()
                    .filter(|l| l.starts_with("processor"))
                    .count() as u32
            })
            .unwrap_or(1)
    }
}

/// Adapter for reading /proc/meminfo.
pub struct MemInfoAdapter;

impl MemInfoAdapter {
    const PATH: &'static str = "/proc/meminfo";

    /// Reads current memory metrics.
    pub fn read() -> ProcfsResult<MemoryMetrics> {
        let content = fs::read_to_string(Self::PATH)
            .map_err(|e| ProcfsError::ReadError(Self::PATH.to_string(), e))?;
        Self::parse(&content)
    }

    /// Parses /proc/meminfo content.
    fn parse(content: &str) -> ProcfsResult<MemoryMetrics> {
        let mut values: HashMap<&str, u64> = HashMap::new();

        for line in content.lines() {
            if let Some((key, rest)) = line.split_once(':') {
                let value = rest
                    .trim()
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
                values.insert(key, value);
            }
        }

        let total_kb = *values.get("MemTotal").unwrap_or(&0);
        let free_kb = *values.get("MemFree").unwrap_or(&0);
        let available_kb = *values.get("MemAvailable").unwrap_or(&free_kb);
        let buffers_kb = *values.get("Buffers").unwrap_or(&0);
        let cached_kb = *values.get("Cached").unwrap_or(&0);
        let dirty_kb = *values.get("Dirty").unwrap_or(&0);
        let swap_total_kb = *values.get("SwapTotal").unwrap_or(&0);
        let swap_free_kb = *values.get("SwapFree").unwrap_or(&0);

        // Convert kB to bytes
        let total_bytes = total_kb * 1024;
        let available_bytes = available_kb * 1024;
        let cached_bytes = (buffers_kb + cached_kb) * 1024;
        let dirty_bytes = dirty_kb * 1024;
        let swap_total_bytes = swap_total_kb * 1024;
        let swap_free_bytes = swap_free_kb * 1024;

        Ok(MemoryMetrics {
            total_bytes,
            available_bytes,
            cached_bytes,
            dirty_bytes,
            swap_total_bytes,
            swap_free_bytes,
            pswpin_delta: 0,  // Will be filled in by sampler with vmstat delta
            pswpout_delta: 0,
        })
    }
}

/// Raw disk stats from /proc/diskstats.
#[derive(Debug, Clone, Default)]
pub struct RawDiskStats {
    /// Device name.
    pub device: String,
    /// Number of reads completed successfully.
    pub reads_completed: u64,
    /// Number of reads merged.
    pub reads_merged: u64,
    /// Number of sectors read.
    pub sectors_read: u64,
    /// Time spent reading (ms).
    pub time_reading_ms: u64,
    /// Number of writes completed successfully.
    pub writes_completed: u64,
    /// Number of writes merged.
    pub writes_merged: u64,
    /// Number of sectors written.
    pub sectors_written: u64,
    /// Time spent writing (ms).
    pub time_writing_ms: u64,
    /// Number of I/Os currently in progress.
    pub io_in_progress: u64,
    /// Time spent doing I/Os (ms).
    pub time_io_ms: u64,
    /// Weighted time spent doing I/Os (ms).
    pub weighted_time_io_ms: u64,
}

/// Adapter for reading /proc/diskstats.
pub struct DiskStatsAdapter;

impl DiskStatsAdapter {
    const PATH: &'static str = "/proc/diskstats";
    const SECTOR_SIZE: u64 = 512; // Standard sector size

    /// Reads all disk stats.
    pub fn read_all() -> ProcfsResult<Vec<RawDiskStats>> {
        let content = fs::read_to_string(Self::PATH)
            .map_err(|e| ProcfsError::ReadError(Self::PATH.to_string(), e))?;
        Self::parse(&content)
    }

    /// Reads stats for physical disks only (filters partitions).
    pub fn read_physical_disks() -> ProcfsResult<Vec<RawDiskStats>> {
        Self::read_all().map(|disks| {
            disks
                .into_iter()
                .filter(|d| Self::is_physical_disk(&d.device))
                .collect()
        })
    }

    /// Checks if device is a physical disk (not a partition).
    fn is_physical_disk(device: &str) -> bool {
        // Include sda, nvme0n1, vda, etc. Exclude sda1, nvme0n1p1, etc.
        if device.starts_with("sd") || device.starts_with("vd") || device.starts_with("hd") {
            // These have partitions as sda1, sda2, etc.
            !device.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false)
                || device.len() == 3
        } else if device.starts_with("nvme") {
            // NVMe partitions are nvme0n1p1, etc.
            !device.contains('p') || device.ends_with("n1") || device.ends_with("n2")
        } else if device.starts_with("loop") || device.starts_with("dm-") || device.starts_with("ram") {
            // Skip loop devices, device mapper, ram disks
            false
        } else {
            false
        }
    }

    /// Parses /proc/diskstats content.
    fn parse(content: &str) -> ProcfsResult<Vec<RawDiskStats>> {
        let mut stats = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 14 {
                continue;
            }

            let parse_u64 = |idx: usize| -> u64 {
                parts.get(idx).and_then(|s| s.parse().ok()).unwrap_or(0)
            };

            stats.push(RawDiskStats {
                device: parts[2].to_string(),
                reads_completed: parse_u64(3),
                reads_merged: parse_u64(4),
                sectors_read: parse_u64(5),
                time_reading_ms: parse_u64(6),
                writes_completed: parse_u64(7),
                writes_merged: parse_u64(8),
                sectors_written: parse_u64(9),
                time_writing_ms: parse_u64(10),
                io_in_progress: parse_u64(11),
                time_io_ms: parse_u64(12),
                weighted_time_io_ms: parse_u64(13),
            });
        }

        Ok(stats)
    }

    /// Calculates IoMetrics from previous and current disk stats.
    pub fn to_metrics(
        prev: &[RawDiskStats],
        curr: &[RawDiskStats],
        interval_ms: u64,
    ) -> IoMetrics {
        use crate::domain::pressure::DeviceIoMetrics;
        
        let prev_map: HashMap<&str, &RawDiskStats> =
            prev.iter().map(|s| (s.device.as_str(), s)).collect();

        let mut devices = Vec::new();
        let mut total_read_bytes_sec = 0u64;
        let mut total_write_bytes_sec = 0u64;
        let interval_sec = (interval_ms as f32) / 1000.0;

        for curr_stat in curr {
            if let Some(prev_stat) = prev_map.get(curr_stat.device.as_str()) {
                let read_sectors = curr_stat.sectors_read.saturating_sub(prev_stat.sectors_read);
                let write_sectors = curr_stat.sectors_written.saturating_sub(prev_stat.sectors_written);
                let time_io = curr_stat.time_io_ms.saturating_sub(prev_stat.time_io_ms);
                let weighted_time = curr_stat.weighted_time_io_ms.saturating_sub(prev_stat.weighted_time_io_ms);

                let read_bytes = read_sectors * Self::SECTOR_SIZE;
                let write_bytes = write_sectors * Self::SECTOR_SIZE;

                // Convert to per-second rates
                let (read_bytes_sec, write_bytes_sec) = if interval_sec > 0.0 {
                    (
                        (read_bytes as f32 / interval_sec) as u64,
                        (write_bytes as f32 / interval_sec) as u64,
                    )
                } else {
                    (0, 0)
                };

                total_read_bytes_sec += read_bytes_sec;
                total_write_bytes_sec += write_bytes_sec;

                devices.push(DeviceIoMetrics {
                    name: curr_stat.device.clone(),
                    read_bytes_sec,
                    write_bytes_sec,
                    io_time_ms: time_io,
                    weighted_io_time_ms: weighted_time,
                });
            }
        }

        IoMetrics {
            devices,
            total_read_bytes_sec,
            total_write_bytes_sec,
        }
    }
}

/// Adapter for reading /proc/vmstat.
pub struct VmStatAdapter;

impl VmStatAdapter {
    const PATH: &'static str = "/proc/vmstat";

    /// Reads vmstat values.
    pub fn read() -> ProcfsResult<HashMap<String, u64>> {
        let content = fs::read_to_string(Self::PATH)
            .map_err(|e| ProcfsError::ReadError(Self::PATH.to_string(), e))?;
        Self::parse(&content)
    }

    /// Parses /proc/vmstat content.
    fn parse(content: &str) -> ProcfsResult<HashMap<String, u64>> {
        let mut values = HashMap::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(value) = parts[1].parse::<u64>() {
                    values.insert(parts[0].to_string(), value);
                }
            }
        }

        Ok(values)
    }

    /// Calculates pages swapped in/out between samples.
    pub fn swap_activity(
        prev: &HashMap<String, u64>,
        curr: &HashMap<String, u64>,
    ) -> (u64, u64) {
        let pswpin_delta = curr
            .get("pswpin")
            .unwrap_or(&0)
            .saturating_sub(*prev.get("pswpin").unwrap_or(&0));
        let pswpout_delta = curr
            .get("pswpout")
            .unwrap_or(&0)
            .saturating_sub(*prev.get("pswpout").unwrap_or(&0));

        (pswpin_delta, pswpout_delta)
    }

    /// Calculates page faults between samples.
    pub fn page_faults(
        prev: &HashMap<String, u64>,
        curr: &HashMap<String, u64>,
    ) -> (u64, u64) {
        let minor = curr
            .get("pgfault")
            .unwrap_or(&0)
            .saturating_sub(*prev.get("pgfault").unwrap_or(&0));
        let major = curr
            .get("pgmajfault")
            .unwrap_or(&0)
            .saturating_sub(*prev.get("pgmajfault").unwrap_or(&0));

        (minor, major)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proc_stat() {
        let content = "cpu  10000 1000 5000 80000 2000 100 50 10 0 0\n\
                       cpu0 5000 500 2500 40000 1000 50 25 5 0 0\n";
        
        let stats = ProcStatAdapter::parse(content).unwrap();
        assert_eq!(stats.user, 10000);
        assert_eq!(stats.nice, 1000);
        assert_eq!(stats.system, 5000);
        assert_eq!(stats.idle, 80000);
        assert_eq!(stats.iowait, 2000);
    }

    #[test]
    fn test_cpu_usage_calculation() {
        let prev = RawCpuStats {
            user: 10000,
            nice: 0,
            system: 5000,
            idle: 80000,
            iowait: 2000,
            ..Default::default()
        };
        let curr = RawCpuStats {
            user: 11000,
            nice: 0,
            system: 5500,
            idle: 82000,
            iowait: 2500,
            ..Default::default()
        };

        let usage = RawCpuStats::usage_percent(&prev, &curr);
        // Delta: user=1000, system=500, idle=2000, iowait=500
        // Total delta = 4000, idle delta = 2500, active = 1500
        // Usage = 1500/4000 = 37.5%
        assert!((usage - 37.5).abs() < 0.1);
    }

    #[test]
    fn test_parse_meminfo() {
        let content = "MemTotal:       16384000 kB\n\
                       MemFree:         4096000 kB\n\
                       MemAvailable:    8192000 kB\n\
                       Buffers:          512000 kB\n\
                       Cached:          2048000 kB\n\
                       SwapTotal:       8192000 kB\n\
                       SwapFree:        8000000 kB\n";
        
        let metrics = MemInfoAdapter::parse(content).unwrap();
        assert_eq!(metrics.total_bytes, 16384000 * 1024);
        assert_eq!(metrics.available_bytes, 8192000 * 1024);
        assert_eq!(metrics.swap_total_bytes, 8192000 * 1024);
    }

    #[test]
    fn test_parse_diskstats() {
        let content = "   8       0 sda 12345 100 98765 5000 54321 200 87654 3000 2 4000 8000 0 0 0 0 0 0\n\
                         8       1 sda1 12000 90 95000 4500 50000 180 80000 2800 0 3800 7300 0 0 0 0 0 0\n";
        
        let stats = DiskStatsAdapter::parse(content).unwrap();
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].device, "sda");
        assert_eq!(stats[0].reads_completed, 12345);
    }

    #[test]
    fn test_is_physical_disk() {
        assert!(DiskStatsAdapter::is_physical_disk("sda"));
        assert!(!DiskStatsAdapter::is_physical_disk("sda1"));
        assert!(DiskStatsAdapter::is_physical_disk("nvme0n1"));
        assert!(!DiskStatsAdapter::is_physical_disk("nvme0n1p1"));
        assert!(!DiskStatsAdapter::is_physical_disk("loop0"));
        assert!(!DiskStatsAdapter::is_physical_disk("dm-0"));
    }
}
