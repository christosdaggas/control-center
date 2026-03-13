//! PSI (Pressure Stall Information) adapter.
//!
//! Reads Linux PSI metrics from /proc/pressure/{cpu,memory,io}.
//! PSI is available on kernels 4.20+ with CONFIG_PSI=y.

use crate::domain::pressure::{PsiMetrics, PsiResource, PsiResourceWithFull};
use std::fs;
use std::path::Path;
use thiserror::Error;
use tracing::debug;

/// Errors from PSI reading.
#[derive(Debug, Error)]
pub enum PsiError {
    /// PSI is not available on this system.
    #[error("PSI not available: {0}")]
    NotAvailable(String),

    /// Failed to read PSI file.
    #[error("Failed to read PSI file: {0}")]
    ReadError(#[from] std::io::Error),

    /// Failed to parse PSI data.
    #[error("Failed to parse PSI data: {0}")]
    ParseError(String),
}

/// Result type for PSI operations.
pub type PsiResult<T> = Result<T, PsiError>;

/// PSI availability status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsiAvailability {
    /// Whether /proc/pressure/cpu exists.
    pub cpu: bool,
    /// Whether /proc/pressure/memory exists.
    pub memory: bool,
    /// Whether /proc/pressure/io exists.
    pub io: bool,
}

impl PsiAvailability {
    /// Returns true if any PSI metric is available.
    #[must_use]
    pub fn any_available(&self) -> bool {
        self.cpu || self.memory || self.io
    }

    /// Returns true if all PSI metrics are available.
    #[must_use]
    pub fn all_available(&self) -> bool {
        self.cpu && self.memory && self.io
    }
}

/// Adapter for reading PSI metrics.
pub struct PsiAdapter;

impl PsiAdapter {
    const CPU_PATH: &'static str = "/proc/pressure/cpu";
    const MEMORY_PATH: &'static str = "/proc/pressure/memory";
    const IO_PATH: &'static str = "/proc/pressure/io";

    /// Checks PSI availability on this system.
    #[must_use]
    pub fn check_availability() -> PsiAvailability {
        PsiAvailability {
            cpu: Path::new(Self::CPU_PATH).exists(),
            memory: Path::new(Self::MEMORY_PATH).exists(),
            io: Path::new(Self::IO_PATH).exists(),
        }
    }

    /// Reads all available PSI metrics.
    pub fn read_all() -> PsiResult<PsiMetrics> {
        let availability = Self::check_availability();

        if !availability.any_available() {
            return Err(PsiError::NotAvailable(
                "No PSI files found in /proc/pressure/".to_string(),
            ));
        }

        let cpu = if availability.cpu {
            Self::read_cpu()?
        } else {
            debug!("PSI CPU not available");
            PsiResource::default()
        };

        let memory = if availability.memory {
            Self::read_memory()?
        } else {
            debug!("PSI Memory not available");
            PsiResourceWithFull::default()
        };

        let io = if availability.io {
            Self::read_io()?
        } else {
            debug!("PSI I/O not available");
            PsiResourceWithFull::default()
        };

        Ok(PsiMetrics { cpu, memory, io })
    }

    /// Reads CPU pressure metrics.
    pub fn read_cpu() -> PsiResult<PsiResource> {
        let content = fs::read_to_string(Self::CPU_PATH)?;
        Self::parse_some_only(&content)
    }

    /// Reads memory pressure metrics.
    pub fn read_memory() -> PsiResult<PsiResourceWithFull> {
        let content = fs::read_to_string(Self::MEMORY_PATH)?;
        Self::parse_some_and_full(&content)
    }

    /// Reads I/O pressure metrics.
    pub fn read_io() -> PsiResult<PsiResourceWithFull> {
        let content = fs::read_to_string(Self::IO_PATH)?;
        Self::parse_some_and_full(&content)
    }

    /// Parses a PSI file with only "some" line (CPU).
    /// Format: some avg10=0.00 avg60=0.00 avg300=0.00 total=123456
    fn parse_some_only(content: &str) -> PsiResult<PsiResource> {
        let mut resource = PsiResource::default();

        for line in content.lines() {
            if line.starts_with("some") {
                Self::parse_psi_line(line, &mut resource.some_avg10, &mut resource.some_avg60,
                                     &mut resource.some_avg300, &mut resource.some_total_us)?;
            }
        }

        Ok(resource)
    }

    /// Parses a PSI file with "some" and "full" lines (memory, io).
    fn parse_some_and_full(content: &str) -> PsiResult<PsiResourceWithFull> {
        let mut resource = PsiResourceWithFull::default();

        for line in content.lines() {
            if line.starts_with("some") {
                Self::parse_psi_line(line, &mut resource.some_avg10, &mut resource.some_avg60,
                                     &mut resource.some_avg300, &mut resource.some_total_us)?;
            } else if line.starts_with("full") {
                Self::parse_psi_line(line, &mut resource.full_avg10, &mut resource.full_avg60,
                                     &mut resource.full_avg300, &mut resource.full_total_us)?;
            }
        }

        Ok(resource)
    }

    /// Parses a single PSI line.
    fn parse_psi_line(
        line: &str,
        avg10: &mut f32,
        avg60: &mut f32,
        avg300: &mut f32,
        total: &mut u64,
    ) -> PsiResult<()> {
        for part in line.split_whitespace().skip(1) {
            if let Some((key, value)) = part.split_once('=') {
                match key {
                    "avg10" => {
                        *avg10 = value.parse().map_err(|_| {
                            PsiError::ParseError(format!("Invalid avg10: {}", value))
                        })?;
                    }
                    "avg60" => {
                        *avg60 = value.parse().map_err(|_| {
                            PsiError::ParseError(format!("Invalid avg60: {}", value))
                        })?;
                    }
                    "avg300" => {
                        *avg300 = value.parse().map_err(|_| {
                            PsiError::ParseError(format!("Invalid avg300: {}", value))
                        })?;
                    }
                    "total" => {
                        *total = value.parse().map_err(|_| {
                            PsiError::ParseError(format!("Invalid total: {}", value))
                        })?;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_psi() {
        let content = "some avg10=1.23 avg60=2.34 avg300=3.45 total=12345678\n";
        let result = PsiAdapter::parse_some_only(content).unwrap();
        
        assert!((result.some_avg10 - 1.23).abs() < 0.01);
        assert!((result.some_avg60 - 2.34).abs() < 0.01);
        assert!((result.some_avg300 - 3.45).abs() < 0.01);
        assert_eq!(result.some_total_us, 12345678);
    }

    #[test]
    fn test_parse_memory_psi() {
        let content = "some avg10=0.50 avg60=0.75 avg300=1.00 total=1000000\n\
                       full avg10=0.10 avg60=0.20 avg300=0.30 total=500000\n";
        let result = PsiAdapter::parse_some_and_full(content).unwrap();
        
        assert!((result.some_avg10 - 0.50).abs() < 0.01);
        assert!((result.full_avg10 - 0.10).abs() < 0.01);
        assert_eq!(result.some_total_us, 1000000);
        assert_eq!(result.full_total_us, 500000);
    }

    #[test]
    fn test_availability_check() {
        // This test just verifies the function runs without panicking
        let avail = PsiAdapter::check_availability();
        // On most modern Linux systems, PSI should be available
        // but we don't assert because this is environment-dependent
        println!("PSI availability: {:?}", avail);
    }
}
