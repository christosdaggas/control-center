//! Time-series ring buffer for pressure samples.
//!
//! Maintains multiple granularities of data:
//! - Fine: 2-second samples, keep last 30 (1 minute of data)
//! - Medium: 30-second averages, keep last 30 (15 minutes of data)
//! - Coarse: 5-minute averages, keep last 288 (24 hours of data)

use crate::domain::pressure::{CpuMetrics, IoMetrics, PressureSample, PsiMetrics};
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use uuid::Uuid;

/// The sample granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleGranularity {
    /// 2-second samples.
    Fine,
    /// 30-second averages.
    Medium,
    /// 5-minute averages.
    Coarse,
}

impl SampleGranularity {
    /// Returns the interval in seconds for this granularity.
    #[must_use]
    pub const fn interval_secs(&self) -> u64 {
        match self {
            Self::Fine => 2,
            Self::Medium => 30,
            Self::Coarse => 300,
        }
    }
}

/// A timestamped sample with metadata.
#[derive(Debug, Clone)]
struct TimestampedSample {
    sample: PressureSample,
}

/// Single-granularity ring buffer.
#[derive(Debug)]
struct GranularityBuffer {
    samples: VecDeque<TimestampedSample>,
    max_size: usize,
    last_aggregation: Option<DateTime<Utc>>,
}

impl GranularityBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_size),
            max_size,
            last_aggregation: None,
        }
    }

    fn push(&mut self, sample: PressureSample) {
        let ts = TimestampedSample {
            sample,
        };

        if self.samples.len() >= self.max_size {
            self.samples.pop_front();
        }
        self.samples.push_back(ts);
    }

    fn latest(&self) -> Option<&PressureSample> {
        self.samples.back().map(|ts| &ts.sample)
    }

    fn iter(&self) -> impl Iterator<Item = &PressureSample> {
        self.samples.iter().map(|ts| &ts.sample)
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn clear(&mut self) {
        self.samples.clear();
        self.last_aggregation = None;
    }
}

/// Multi-granularity ring buffer for pressure samples.
#[derive(Debug)]
pub struct PressureRingBuffer {
    fine: GranularityBuffer,
    medium: GranularityBuffer,
    coarse: GranularityBuffer,
    fine_accumulator: Vec<PressureSample>,
    medium_accumulator: Vec<PressureSample>,
}

impl PressureRingBuffer {
    /// Fine buffer: 2s samples, 30 samples = 1 minute.

    const FINE_SIZE: usize = 30;

    /// Medium buffer: 30s averages, 30 samples = 15 minutes.

    const MEDIUM_SIZE: usize = 30;

    /// Coarse buffer: 5m averages, 288 samples = 24 hours.

    const COARSE_SIZE: usize = 288;

    /// Creates a new ring buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fine: GranularityBuffer::new(Self::FINE_SIZE),
            medium: GranularityBuffer::new(Self::MEDIUM_SIZE),
            coarse: GranularityBuffer::new(Self::COARSE_SIZE),
            fine_accumulator: Vec::with_capacity(15), // 30s / 2s
            medium_accumulator: Vec::with_capacity(10), // 5m / 30s
        }
    }

    /// Pushes a new fine-grained sample.
    pub fn push(&mut self, sample: PressureSample) {
        // Add to fine buffer
        self.fine.push(sample.clone());

        // Accumulate for medium aggregation
        self.fine_accumulator.push(sample);

        // Every 15 fine samples (30 seconds), aggregate to medium
        if self.fine_accumulator.len() >= 15 {
            if let Some(avg) = Self::average_samples(&self.fine_accumulator) {
                self.medium.push(avg.clone());
                self.medium_accumulator.push(avg);
            }
            self.fine_accumulator.clear();
        }

        // Every 10 medium samples (5 minutes), aggregate to coarse
        if self.medium_accumulator.len() >= 10 {
            if let Some(avg) = Self::average_samples(&self.medium_accumulator) {
                self.coarse.push(avg);
            }
            self.medium_accumulator.clear();
        }
    }

    /// Returns the latest sample at the given granularity.
    #[must_use]
    pub fn latest(&self, granularity: SampleGranularity) -> Option<&PressureSample> {
        match granularity {
            SampleGranularity::Fine => self.fine.latest(),
            SampleGranularity::Medium => self.medium.latest(),
            SampleGranularity::Coarse => self.coarse.latest(),
        }
    }

    /// Returns an iterator over samples at the given granularity.
    pub fn iter(&self, granularity: SampleGranularity) -> impl Iterator<Item = &PressureSample> {
        match granularity {
            SampleGranularity::Fine => Box::new(self.fine.iter()) as Box<dyn Iterator<Item = _>>,
            SampleGranularity::Medium => Box::new(self.medium.iter()),
            SampleGranularity::Coarse => Box::new(self.coarse.iter()),
        }
    }

    /// Returns samples within a time range.
    pub fn range(
        &self,
        granularity: SampleGranularity,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<&PressureSample> {
        self.iter(granularity)
            .filter(|s| s.timestamp >= from && s.timestamp <= to)
            .collect()
    }

    /// Returns the last N samples at the given granularity.
    pub fn last_n(&self, granularity: SampleGranularity, n: usize) -> Vec<&PressureSample> {
        let buffer = match granularity {
            SampleGranularity::Fine => &self.fine,
            SampleGranularity::Medium => &self.medium,
            SampleGranularity::Coarse => &self.coarse,
        };
        buffer.samples.iter().rev().take(n).map(|ts| &ts.sample).collect()
    }

    /// Returns the average of the last N samples.
    pub fn average_last_n(
        &self,
        granularity: SampleGranularity,
        n: usize,
    ) -> Option<PressureSample> {
        let samples = self.last_n(granularity, n);
        Self::average_samples_ref(&samples)
    }

    /// Returns buffer statistics.
    #[must_use]
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            fine_count: self.fine.len(),
            fine_max: Self::FINE_SIZE,
            medium_count: self.medium.len(),
            medium_max: Self::MEDIUM_SIZE,
            coarse_count: self.coarse.len(),
            coarse_max: Self::COARSE_SIZE,
        }
    }

    /// Clears all buffers.
    pub fn clear(&mut self) {
        self.fine.clear();
        self.medium.clear();
        self.coarse.clear();
        self.fine_accumulator.clear();
        self.medium_accumulator.clear();
    }

    /// Averages multiple samples into one.
    fn average_samples(samples: &[PressureSample]) -> Option<PressureSample> {
        if samples.is_empty() {
            return None;
        }
        Self::average_samples_ref(&samples.iter().collect::<Vec<_>>())
    }

    /// Averages multiple sample references into one.
    fn average_samples_ref(samples: &[&PressureSample]) -> Option<PressureSample> {
        let count = samples.len() as f32;
        if count == 0.0 {
            return None;
        }

        // Average CPU metrics
        let mut cpu = CpuMetrics::default();
        for s in samples {
            cpu.utilization += s.cpu.utilization;
            cpu.iowait += s.cpu.iowait;
            cpu.system += s.cpu.system;
            cpu.user += s.cpu.user;
            cpu.load_1m += s.cpu.load_1m;
            cpu.load_5m += s.cpu.load_5m;
        }
        cpu.utilization /= count;
        cpu.iowait /= count;
        cpu.system /= count;
        cpu.user /= count;
        cpu.load_1m /= count;
        cpu.load_5m /= count;
        cpu.runnable = samples.last().map(|s| s.cpu.runnable).unwrap_or(0);

        // Use latest memory metrics (absolute values)
        let memory = samples.last().map(|s| s.memory.clone()).unwrap_or_default();

        // Average I/O metrics
        let mut io = IoMetrics::default();
        for s in samples {
            io.total_read_bytes_sec += s.io.total_read_bytes_sec;
            io.total_write_bytes_sec += s.io.total_write_bytes_sec;
        }
        io.total_read_bytes_sec = (io.total_read_bytes_sec as f32 / count) as u64;
        io.total_write_bytes_sec = (io.total_write_bytes_sec as f32 / count) as u64;
        io.devices = samples.last().map(|s| s.io.devices.clone()).unwrap_or_default();

        // Average PSI metrics if present
        let psi = {
            let psi_samples: Vec<_> = samples.iter().filter_map(|s| s.psi.as_ref()).collect();
            if psi_samples.is_empty() {
                None
            } else {
                let psi_count = psi_samples.len() as f32;
                let mut psi = PsiMetrics::default();
                for p in &psi_samples {
                    psi.cpu.some_avg10 += p.cpu.some_avg10;
                    psi.cpu.some_avg60 += p.cpu.some_avg60;
                    psi.cpu.some_avg300 += p.cpu.some_avg300;
                    psi.memory.some_avg10 += p.memory.some_avg10;
                    psi.memory.full_avg10 += p.memory.full_avg10;
                    psi.io.some_avg10 += p.io.some_avg10;
                    psi.io.full_avg10 += p.io.full_avg10;
                }
                psi.cpu.some_avg10 /= psi_count;
                psi.cpu.some_avg60 /= psi_count;
                psi.cpu.some_avg300 /= psi_count;
                psi.memory.some_avg10 /= psi_count;
                psi.memory.full_avg10 /= psi_count;
                psi.io.some_avg10 /= psi_count;
                psi.io.full_avg10 /= psi_count;
                Some(psi)
            }
        };

        Some(PressureSample {
            id: Uuid::new_v4(),
            timestamp: samples.last().map(|s| s.timestamp).unwrap_or_else(Utc::now),
            cpu,
            memory,
            io,
            psi,
        })
    }
}

impl Default for PressureRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about buffer usage.
#[derive(Debug, Clone, Copy)]
pub struct BufferStats {
    /// Number of fine-grained samples stored.
    pub fine_count: usize,
    /// Maximum fine-grained samples.
    pub fine_max: usize,
    /// Number of medium-grained samples stored.
    pub medium_count: usize,
    /// Maximum medium-grained samples.
    pub medium_max: usize,
    /// Number of coarse-grained samples stored.
    pub coarse_count: usize,
    /// Maximum coarse-grained samples.
    pub coarse_max: usize,
}

impl BufferStats {
    /// Returns total number of samples across all granularities.
    #[must_use]
    pub fn total_samples(&self) -> usize {
        self.fine_count + self.medium_count + self.coarse_count
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pressure::MemoryMetrics;
    use chrono::Utc;

    fn make_sample(cpu_utilization: f32) -> PressureSample {
        PressureSample {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            cpu: CpuMetrics {
                utilization: cpu_utilization,
                iowait: 0.0,
                system: 0.0,
                user: cpu_utilization,
                load_1m: 0.0,
                load_5m: 0.0,
                runnable: 1,
            },
            memory: MemoryMetrics::default(),
            io: IoMetrics::default(),
            psi: None,
        }
    }

    #[test]
    fn test_fine_buffer() {
        let mut buffer = PressureRingBuffer::new();
        
        for i in 0..35 {
            buffer.push(make_sample(i as f32));
        }

        // Should only keep last 30
        assert_eq!(buffer.stats().fine_count, 30);
        
        // Latest should be the last pushed
        let latest = buffer.latest(SampleGranularity::Fine).unwrap();
        assert_eq!(latest.cpu.utilization, 34.0);
    }

    #[test]
    fn test_medium_aggregation() {
        let mut buffer = PressureRingBuffer::new();
        
        // Push 15 samples (triggers medium aggregation)
        for i in 0..15 {
            buffer.push(make_sample((i * 2) as f32));
        }

        assert_eq!(buffer.stats().medium_count, 1);
    }

    #[test]
    fn test_average_samples() {
        let samples = vec![
            make_sample(10.0),
            make_sample(20.0),
            make_sample(30.0),
        ];

        let avg = PressureRingBuffer::average_samples(&samples).unwrap();
        assert!((avg.cpu.utilization - 20.0).abs() < 0.01);
    }
}
