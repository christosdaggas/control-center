//! Pressure metrics infrastructure adapters.
//!
//! This module provides adapters for reading system pressure and performance
//! metrics from various Linux kernel interfaces.

mod psi;
mod procfs;
mod ring_buffer;
mod sampler;
mod unit_mapper;

pub use psi::{PsiAdapter, PsiAvailability, PsiError, PsiResult};
pub use procfs::{DiskStatsAdapter, MemInfoAdapter, ProcStatAdapter, RawCpuStats, RawDiskStats, VmStatAdapter};
pub use ring_buffer::{BufferStats, PressureRingBuffer, SampleGranularity};
pub use sampler::{
    create_shared, PressureSampler, SamplerCapabilities, SamplerError, SamplerResult,
    SharedRingBuffer, SharedSampler,
};
pub use unit_mapper::{ProcessStats, SystemdUnit, UnitMapper, UnitStats};
