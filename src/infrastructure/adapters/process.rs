//! Process information reader from /proc filesystem.
//!
//! Reads per-process CPU, memory, and I/O statistics for the
//! top-N resource consumer drilldown on the Performance page.

use std::fs;
use std::path::Path;
use tracing::debug;

/// Information about a single process.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Process ID.
    pub pid: u32,
    /// Process name (comm).
    pub name: String,
    /// Command line (truncated).
    pub cmdline: String,
    /// CPU usage percentage (0.0 - 100.0 per core).
    pub cpu_percent: f64,
    /// Resident set size in bytes.
    pub rss_bytes: u64,
    /// Virtual memory size in bytes.
    pub vsize_bytes: u64,
    /// Memory usage percentage.
    pub mem_percent: f64,
    /// Process state character (R, S, D, Z, T, etc.).
    pub state: char,
    /// User ID of the process owner.
    pub uid: u32,
}

/// Snapshot of process CPU times for delta calculation.
#[derive(Debug, Clone)]
struct ProcCpuSnapshot {
    pid: u32,
    name: String,
    cmdline: String,
    utime: u64,
    stime: u64,
    rss_pages: u64,
    vsize: u64,
    state: char,
    uid: u32,
}

/// Reads the total system CPU time from /proc/stat.
fn read_total_cpu_ticks() -> Option<u64> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let cpu_line = stat.lines().next()?;
    if !cpu_line.starts_with("cpu ") {
        return None;
    }
    let total: u64 = cpu_line
        .split_whitespace()
        .skip(1)
        .filter_map(|v| v.parse::<u64>().ok())
        .sum();
    Some(total)
}

/// Reads total physical memory in bytes from /proc/meminfo.
fn read_total_memory() -> Option<u64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            let kb: u64 = line
                .split_whitespace()
                .nth(1)?
                .parse()
                .ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

/// Reads per-process snapshot from /proc/[pid]/stat and /proc/[pid]/cmdline.
fn read_proc_snapshot(pid: u32) -> Option<ProcCpuSnapshot> {
    let stat_path = format!("/proc/{}/stat", pid);
    let stat_content = fs::read_to_string(&stat_path).ok()?;

    // Parse /proc/[pid]/stat: pid (comm) state ppid pgrp ... utime stime ...
    // comm can contain spaces and parens, so find the last ')' first
    let comm_start = stat_content.find('(')?;
    let comm_end = stat_content.rfind(')')?;
    let name = stat_content[comm_start + 1..comm_end].to_string();

    let after_comm = &stat_content[comm_end + 2..];
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    if fields.len() < 22 {
        return None;
    }

    let state = fields[0].chars().next().unwrap_or('?');
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    let vsize: u64 = fields[20].parse().ok()?;
    let rss_pages: u64 = fields[21].parse().ok()?;

    // Read UID from /proc/[pid]/status
    let uid = read_proc_uid(pid).unwrap_or(0);

    // Read cmdline
    let cmdline_path = format!("/proc/{}/cmdline", pid);
    let cmdline = fs::read_to_string(&cmdline_path)
        .unwrap_or_default()
        .replace('\0', " ")
        .trim()
        .chars()
        .take(120)
        .collect::<String>();

    Some(ProcCpuSnapshot {
        pid,
        name,
        cmdline,
        utime,
        stime,
        rss_pages,
        vsize,
        state,
        uid,
    })
}

/// Reads the UID of a process from /proc/[pid]/status.
fn read_proc_uid(pid: u32) -> Option<u32> {
    let status = fs::read_to_string(format!("/proc/{}/status", pid)).ok()?;
    for line in status.lines() {
        if line.starts_with("Uid:") {
            return line.split_whitespace().nth(1)?.parse().ok();
        }
    }
    None
}

/// Lists all numeric PIDs in /proc.
fn list_pids() -> Vec<u32> {
    let proc_dir = Path::new("/proc");
    let mut pids = Vec::new();
    if let Ok(entries) = fs::read_dir(proc_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(pid) = name.parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }
    pids
}

/// Adapter for reading top-N resource-consuming processes.
///
/// Uses two-sample delta measurement for accurate CPU percentages.
pub struct ProcessAdapter {
    /// Previous snapshot for CPU delta calculation.
    prev_snapshots: Vec<ProcCpuSnapshot>,
    /// Previous total CPU ticks.
    prev_total_cpu: u64,
    /// Page size in bytes (from sysconf).
    page_size: u64,
}

impl ProcessAdapter {
    /// Creates a new process adapter, taking an initial CPU snapshot.
    pub fn new() -> Self {
        let page_size = 4096_u64; // Standard page size on most Linux systems
        // Try to get actual page size from /proc/self/smaps or use default
        if let Ok(contents) = std::fs::read_to_string("/proc/self/smaps") {
            // Just use the default, sysconf requires unsafe
            let _ = contents;
        }
        let pids = list_pids();
        let snapshots: Vec<ProcCpuSnapshot> = pids
            .iter()
            .filter_map(|pid| read_proc_snapshot(*pid))
            .collect();
        let total_cpu = read_total_cpu_ticks().unwrap_or(1);

        debug!(
            process_count = snapshots.len(),
            "ProcessAdapter initialized"
        );

        Self {
            prev_snapshots: snapshots,
            prev_total_cpu: total_cpu,
            page_size,
        }
    }

    /// Takes a new sample and returns the top N processes by CPU usage.
    pub fn top_by_cpu(&mut self, n: usize) -> Vec<ProcessInfo> {
        let mut result = self.sample();
        result.sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap_or(std::cmp::Ordering::Equal));
        result.truncate(n);
        result
    }

    /// Takes a new sample and returns the top N processes by memory usage.
    pub fn top_by_memory(&mut self, n: usize) -> Vec<ProcessInfo> {
        let mut result = self.sample();
        result.sort_by(|a, b| b.rss_bytes.partial_cmp(&a.rss_bytes).unwrap_or(std::cmp::Ordering::Equal));
        result.truncate(n);
        result
    }

    /// Samples all processes and computes CPU deltas.
    fn sample(&mut self) -> Vec<ProcessInfo> {
        let total_mem = read_total_memory().unwrap_or(1);
        let new_total_cpu = read_total_cpu_ticks().unwrap_or(1);
        let cpu_delta = new_total_cpu.saturating_sub(self.prev_total_cpu).max(1);

        let pids = list_pids();
        let new_snapshots: Vec<ProcCpuSnapshot> = pids
            .iter()
            .filter_map(|pid| read_proc_snapshot(*pid))
            .collect();

        // Build lookup from previous snapshots
        let prev_map: std::collections::HashMap<u32, &ProcCpuSnapshot> = self
            .prev_snapshots
            .iter()
            .map(|s| (s.pid, s))
            .collect();

        let mut result = Vec::with_capacity(new_snapshots.len());
        for snap in &new_snapshots {
            let cpu_percent = if let Some(prev) = prev_map.get(&snap.pid) {
                let proc_delta = (snap.utime + snap.stime)
                    .saturating_sub(prev.utime + prev.stime);
                (proc_delta as f64 / cpu_delta as f64) * 100.0
            } else {
                0.0
            };

            let rss_bytes = snap.rss_pages * self.page_size;
            let mem_percent = (rss_bytes as f64 / total_mem as f64) * 100.0;

            result.push(ProcessInfo {
                pid: snap.pid,
                name: snap.name.clone(),
                cmdline: if snap.cmdline.is_empty() {
                    format!("[{}]", snap.name)
                } else {
                    snap.cmdline.clone()
                },
                cpu_percent,
                rss_bytes,
                vsize_bytes: snap.vsize,
                mem_percent,
                state: snap.state,
                uid: snap.uid,
            });
        }

        // Save current snapshot for next delta
        self.prev_snapshots = new_snapshots;
        self.prev_total_cpu = new_total_cpu;

        result
    }
}

/// Formats bytes into a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_list_pids_returns_some() {
        let pids = list_pids();
        // PID 1 (init/systemd) should always exist
        assert!(!pids.is_empty());
        assert!(pids.contains(&1));
    }

    #[test]
    fn test_read_total_cpu_ticks() {
        let ticks = read_total_cpu_ticks();
        assert!(ticks.is_some());
        assert!(ticks.unwrap() > 0);
    }
}
