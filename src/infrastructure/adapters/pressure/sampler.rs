//! Unified pressure sampler that collects from all sources.

use super::procfs::{DiskStatsAdapter, MemInfoAdapter, ProcStatAdapter, RawCpuStats, RawDiskStats};
use super::psi::{PsiAdapter, PsiAvailability};
use super::ring_buffer::PressureRingBuffer;
use crate::domain::pressure::{
    CpuMetrics, DeviceIoMetrics, IoMetrics, MemoryMetrics, PressureSample, PsiMetrics,
    PsiResource, PsiResourceWithFull,
};
use chrono::Utc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;

/// Errors that can occur during pressure sampling.
#[derive(Debug, Error)]
pub enum SamplerError {
    /// No data sources are available.
    #[error("No pressure data sources available")]
    NoSources,
    /// Failed to read pressure data.
    #[error("Failed to read pressure data: {0}")]
    ReadError(String),
}

/// Result type for sampler operations.
pub type SamplerResult<T> = Result<T, SamplerError>;

/// Describes what data sources are available for sampling.
#[derive(Debug, Clone)]
pub struct SamplerCapabilities {
    /// PSI availability per resource.
    pub psi: PsiAvailability,
    /// Whether /proc/stat is readable.
    pub proc_stat: bool,
    /// Whether /proc/meminfo is readable.
    pub proc_meminfo: bool,
    /// Whether /proc/diskstats is readable.
    pub proc_diskstats: bool,
    /// Whether /proc/loadavg is readable.
    pub proc_loadavg: bool,
    /// Number of CPU cores detected.
    pub cpu_cores: u32,
}

impl SamplerCapabilities {
    /// Detects available capabilities by probing the system.
    #[must_use]
    pub fn detect() -> Self {
        let psi = PsiAdapter::check_availability();
        let proc_stat = ProcStatAdapter::read().is_ok();
        let proc_meminfo = MemInfoAdapter::read().is_ok();
        let proc_diskstats = DiskStatsAdapter::read_all().is_ok();
        let proc_loadavg = ProcStatAdapter::read_load_avg().is_ok();
        let cpu_cores = ProcStatAdapter::cpu_core_count();

        let caps = Self {
            psi,
            proc_stat,
            proc_meminfo,
            proc_diskstats,
            proc_loadavg,
            cpu_cores,
        };

        info!("Sampler capabilities detected: {:?}", caps);
        caps
    }

    /// Returns true if basic CPU and memory metrics are available.
    #[must_use]
    pub fn has_basic_metrics(&self) -> bool {
        self.proc_stat && self.proc_meminfo
    }

    /// Returns a human-readable summary of available capabilities.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut features = Vec::new();
        if self.psi.all_available() {
            features.push("PSI (full)");
        } else if self.psi.any_available() {
            features.push("PSI (partial)");
        }
        if self.proc_stat { features.push("CPU stats"); }
        if self.proc_meminfo { features.push("Memory stats"); }
        if self.proc_diskstats { features.push("Disk stats"); }

        if features.is_empty() {
            "No features available".to_string()
        } else {
            features.join(", ")
        }
    }
}

#[derive(Debug, Default)]
struct PreviousSample {
    cpu_stats: Option<RawCpuStats>,
    disk_stats: Option<Vec<RawDiskStats>>,
    instant: Option<Instant>,
}

/// Unified pressure sampler that collects metrics from all available sources.
pub struct PressureSampler {
    capabilities: SamplerCapabilities,
    previous: RwLock<PreviousSample>,
}

impl PressureSampler {
    /// Creates a new sampler with auto-detected capabilities.
    #[must_use]
    pub fn new() -> Self {
        let capabilities = SamplerCapabilities::detect();
        Self {
            capabilities,
            previous: RwLock::new(PreviousSample::default()),
        }
    }

    /// Creates a new sampler with explicit capabilities.
    #[must_use]
    pub fn with_capabilities(capabilities: SamplerCapabilities) -> Self {
        Self {
            capabilities,
            previous: RwLock::new(PreviousSample::default()),
        }
    }

    /// Returns the detected capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &SamplerCapabilities {
        &self.capabilities
    }

    /// Takes a pressure sample from all available sources.
    pub fn sample(&self) -> SamplerResult<PressureSample> {
        let now = Instant::now();

        let current_cpu = if self.capabilities.proc_stat {
            ProcStatAdapter::read().ok()
        } else {
            None
        };

        let current_disk = if self.capabilities.proc_diskstats {
            DiskStatsAdapter::read_physical_disks().ok()
        } else {
            None
        };

        let mut prev = self.previous.write().map_err(|_| {
            SamplerError::ReadError("Failed to acquire lock".to_string())
        })?;

        // CPU metrics
        let (load_1m, load_5m, _) = ProcStatAdapter::read_load_avg().unwrap_or((0.0, 0.0, 0.0));
        let runnable = ProcStatAdapter::read_runnable_tasks().unwrap_or(0);

        let (utilization, iowait, system, user) = match (&prev.cpu_stats, &current_cpu) {
            (Some(prev_cpu), Some(curr_cpu)) => {
                let total_delta = curr_cpu.total().saturating_sub(prev_cpu.total());
                let idle_delta = curr_cpu.idle_total().saturating_sub(prev_cpu.idle_total());
                let iowait_delta = curr_cpu.iowait.saturating_sub(prev_cpu.iowait);
                let system_delta = curr_cpu.system.saturating_sub(prev_cpu.system);
                let user_delta = curr_cpu.user.saturating_sub(prev_cpu.user);

                if total_delta > 0 {
                    let active = total_delta.saturating_sub(idle_delta);
                    (
                        (active as f32 / total_delta as f32) * 100.0,
                        (iowait_delta as f32 / total_delta as f32) * 100.0,
                        (system_delta as f32 / total_delta as f32) * 100.0,
                        (user_delta as f32 / total_delta as f32) * 100.0,
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0)
                }
            }
            _ => (0.0, 0.0, 0.0, 0.0),
        };

        let cpu = CpuMetrics {
            utilization,
            iowait,
            system,
            user,
            load_1m,
            load_5m,
            runnable,
        };

        // Memory metrics
        let memory = if self.capabilities.proc_meminfo {
            let raw = MemInfoAdapter::read().unwrap_or_default();
            MemoryMetrics {
                total_bytes: raw.total_bytes,
                available_bytes: raw.available_bytes,
                cached_bytes: raw.cached_bytes,
                dirty_bytes: raw.dirty_bytes,
                swap_total_bytes: raw.swap_total_bytes,
                swap_free_bytes: raw.swap_free_bytes,
                pswpin_delta: 0,
                pswpout_delta: 0,
            }
        } else {
            MemoryMetrics::default()
        };

        // I/O metrics
        let io = match (&prev.disk_stats, &current_disk, prev.instant) {
            (Some(prev_disks), Some(curr_disks), Some(prev_instant)) => {
                let interval_ms = now.duration_since(prev_instant).as_millis() as u64;
                self.calculate_io_metrics(prev_disks, curr_disks, interval_ms)
            }
            _ => IoMetrics::default(),
        };

        // PSI metrics
        let psi = if self.capabilities.psi.any_available() {
            match PsiAdapter::read_all() {
                Ok(raw_psi) => Some(PsiMetrics {
                    cpu: PsiResource {
                        some_avg10: raw_psi.cpu.some_avg10,
                        some_avg60: raw_psi.cpu.some_avg60,
                        some_avg300: raw_psi.cpu.some_avg300,
                        some_total_us: raw_psi.cpu.some_total_us,
                    },
                    memory: PsiResourceWithFull {
                        some_avg10: raw_psi.memory.some_avg10,
                        some_avg60: raw_psi.memory.some_avg60,
                        some_avg300: raw_psi.memory.some_avg300,
                        some_total_us: raw_psi.memory.some_total_us,
                        full_avg10: raw_psi.memory.full_avg10,
                        full_avg60: raw_psi.memory.full_avg60,
                        full_avg300: raw_psi.memory.full_avg300,
                        full_total_us: raw_psi.memory.full_total_us,
                    },
                    io: PsiResourceWithFull {
                        some_avg10: raw_psi.io.some_avg10,
                        some_avg60: raw_psi.io.some_avg60,
                        some_avg300: raw_psi.io.some_avg300,
                        some_total_us: raw_psi.io.some_total_us,
                        full_avg10: raw_psi.io.full_avg10,
                        full_avg60: raw_psi.io.full_avg60,
                        full_avg300: raw_psi.io.full_avg300,
                        full_total_us: raw_psi.io.full_total_us,
                    },
                }),
                Err(_) => None,
            }
        } else {
            None
        };

        // Update previous
        prev.cpu_stats = current_cpu;
        prev.disk_stats = current_disk;
        prev.instant = Some(now);
        drop(prev);

        Ok(PressureSample {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            cpu,
            memory,
            io,
            psi,
        })
    }

    fn calculate_io_metrics(
        &self,
        prev: &[RawDiskStats],
        curr: &[RawDiskStats],
        interval_ms: u64,
    ) -> IoMetrics {
        use std::collections::HashMap;
        let prev_map: HashMap<&str, &RawDiskStats> =
            prev.iter().map(|s| (s.device.as_str(), s)).collect();

        let mut devices = Vec::new();
        let mut total_read = 0u64;
        let mut total_write = 0u64;

        for curr_stat in curr {
            if let Some(prev_stat) = prev_map.get(curr_stat.device.as_str()) {
                let read_sectors = curr_stat.sectors_read.saturating_sub(prev_stat.sectors_read);
                let write_sectors = curr_stat.sectors_written.saturating_sub(prev_stat.sectors_written);
                let time_io = curr_stat.time_io_ms.saturating_sub(prev_stat.time_io_ms);
                let weighted_io = curr_stat.weighted_time_io_ms.saturating_sub(prev_stat.weighted_time_io_ms);

                let sector_size = 512u64;
                let read_bytes = read_sectors * sector_size;
                let write_bytes = write_sectors * sector_size;

                let interval_sec = (interval_ms as f64) / 1000.0;
                let (read_per_sec, write_per_sec) = if interval_sec > 0.0 {
                    (
                        (read_bytes as f64 / interval_sec) as u64,
                        (write_bytes as f64 / interval_sec) as u64,
                    )
                } else {
                    (0, 0)
                };

                total_read += read_per_sec;
                total_write += write_per_sec;

                devices.push(DeviceIoMetrics {
                    name: curr_stat.device.clone(),
                    read_bytes_sec: read_per_sec,
                    write_bytes_sec: write_per_sec,
                    io_time_ms: time_io,
                    weighted_io_time_ms: weighted_io,
                });
            }
        }

        IoMetrics {
            devices,
            total_read_bytes_sec: total_read,
            total_write_bytes_sec: total_write,
        }
    }

    /// Primes the sampler with initial readings for delta calculations.
    pub fn prime(&self) -> SamplerResult<()> {
        if let Ok(cpu_stats) = ProcStatAdapter::read() {
            if let Ok(mut prev) = self.previous.write() {
                prev.cpu_stats = Some(cpu_stats);
            }
        }

        if let Ok(disk_stats) = DiskStatsAdapter::read_physical_disks() {
            if let Ok(mut prev) = self.previous.write() {
                prev.disk_stats = Some(disk_stats);
                prev.instant = Some(Instant::now());
            }
        }

        debug!("Sampler primed with initial readings");
        Ok(())
    }
}

impl Default for PressureSampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared sampler.
pub type SharedSampler = Arc<PressureSampler>;
/// Thread-safe shared ring buffer.
pub type SharedRingBuffer = Arc<RwLock<PressureRingBuffer>>;

/// Creates a new shared sampler and ring buffer pair.
#[must_use]
pub fn create_shared() -> (SharedSampler, SharedRingBuffer) {
    let sampler = Arc::new(PressureSampler::new());
    let buffer = Arc::new(RwLock::new(PressureRingBuffer::new()));
    (sampler, buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_detect() {
        let caps = SamplerCapabilities::detect();
        println!("Detected capabilities: {:?}", caps);
        println!("Summary: {}", caps.summary());
    }

    #[test]
    fn test_sampler_prime() {
        let sampler = PressureSampler::new();
        let result = sampler.prime();
        println!("Prime result: {:?}", result);
    }
}
