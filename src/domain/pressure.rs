//! Resource pressure and bottleneck domain models.
//!
//! This module defines the core types for diagnosing system resource pressure
//! and identifying bottlenecks. All logic is deterministic and rule-based.
//!
//! **Important**: This module must not depend on UI or platform-specific crates.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Classification of system bottleneck types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BottleneckType {
    /// CPU is the primary constraint (high utilization, low iowait).
    CpuBound,
    /// Memory pressure is high (low available, high swap, possible OOM risk).
    MemoryPressure,
    /// I/O is the primary constraint (high iowait, disk saturation).
    IoBound,
    /// Network issues suspected (timeouts, DNS failures, link flaps).
    NetworkSuspected,
    /// Multiple resources are constrained simultaneously.
    MultiFactor,
    /// No clear bottleneck detected; system appears healthy.
    NoClearBottleneck,
}

impl BottleneckType {
    /// Returns a human-readable label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::CpuBound => "CPU Bound",
            Self::MemoryPressure => "Memory Pressure",
            Self::IoBound => "I/O Bound",
            Self::NetworkSuspected => "Network Issues",
            Self::MultiFactor => "Multiple Factors",
            Self::NoClearBottleneck => "No Bottleneck",
        }
    }

    /// Returns an icon name for this bottleneck type.
    #[must_use]
    pub const fn icon_name(&self) -> &'static str {
        match self {
            Self::CpuBound => "processor-symbolic",
            Self::MemoryPressure => "memory-symbolic",
            Self::IoBound => "drive-harddisk-symbolic",
            Self::NetworkSuspected => "network-error-symbolic",
            Self::MultiFactor => "dialog-warning-symbolic",
            Self::NoClearBottleneck => "checkmark-symbolic",
        }
    }

    /// Returns a CSS class for styling.
    #[must_use]
    pub const fn css_class(&self) -> &'static str {
        match self {
            Self::CpuBound => "bottleneck-cpu",
            Self::MemoryPressure => "bottleneck-memory",
            Self::IoBound => "bottleneck-io",
            Self::NetworkSuspected => "bottleneck-network",
            Self::MultiFactor => "bottleneck-multi",
            Self::NoClearBottleneck => "bottleneck-none",
        }
    }
}

impl std::fmt::Display for BottleneckType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Confidence level for a diagnosis (0-100).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Confidence(u8);

impl Confidence {
    /// Creates a new confidence value, clamped to 0-100.
    #[must_use]
    pub fn new(value: u8) -> Self {
        Self(value.min(100))
    }

    /// Returns the raw confidence value.
    #[must_use]
    pub const fn value(&self) -> u8 {
        self.0
    }

    /// Returns a human-readable confidence label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self.0 {
            0..=30 => "Low",
            31..=60 => "Medium",
            61..=85 => "High",
            86..=100 => "Very High",
            _ => "Unknown",
        }
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self(0)
    }
}

/// A complete diagnosis result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    /// Unique identifier for this diagnosis.
    pub id: Uuid,
    /// When the diagnosis was computed.
    pub computed_at: DateTime<Utc>,
    /// Primary bottleneck classification.
    pub bottleneck_type: BottleneckType,
    /// Confidence in the classification (0-100).
    pub confidence: Confidence,
    /// One-sentence summary for display.
    pub summary: String,
    /// Rules that fired to produce this diagnosis.
    pub rules_fired: Vec<RuleMatch>,
    /// Top contributors to the bottleneck.
    pub contributors: Vec<Contributor>,
    /// Time window analyzed.
    pub time_window: TimeWindow,
    /// Related timeline events (by reference).
    pub related_events: Vec<EventRef>,
    /// Available data sources used.
    pub data_sources: Vec<DataSource>,
    /// Any limitations due to missing data.
    pub limitations: Vec<String>,
}

impl Diagnosis {
    /// Creates a new diagnosis builder.
    #[must_use]
    pub fn builder(bottleneck_type: BottleneckType) -> DiagnosisBuilder {
        DiagnosisBuilder::new(bottleneck_type)
    }
}

/// Builder for constructing a Diagnosis.
pub struct DiagnosisBuilder {
    bottleneck_type: BottleneckType,
    confidence: Confidence,
    summary: Option<String>,
    rules_fired: Vec<RuleMatch>,
    contributors: Vec<Contributor>,
    time_window: Option<TimeWindow>,
    related_events: Vec<EventRef>,
    data_sources: Vec<DataSource>,
    limitations: Vec<String>,
}

impl DiagnosisBuilder {
    fn new(bottleneck_type: BottleneckType) -> Self {
        Self {
            bottleneck_type,
            confidence: Confidence::default(),
            summary: None,
            rules_fired: Vec::new(),
            contributors: Vec::new(),
            time_window: None,
            related_events: Vec::new(),
            data_sources: Vec::new(),
            limitations: Vec::new(),
        }
    }

    /// Sets the confidence level.
    #[must_use]
    pub fn confidence(mut self, value: u8) -> Self {
        self.confidence = Confidence::new(value);
        self
    }

    /// Sets the summary sentence.
    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Adds a rule match.
    #[must_use]
    pub fn rule(mut self, rule: RuleMatch) -> Self {
        self.rules_fired.push(rule);
        self
    }

    /// Adds a contributor.
    #[must_use]
    pub fn contributor(mut self, contributor: Contributor) -> Self {
        self.contributors.push(contributor);
        self
    }

    /// Sets the time window.
    #[must_use]
    pub fn time_window(mut self, window: TimeWindow) -> Self {
        self.time_window = Some(window);
        self
    }

    /// Adds a related event reference.
    #[must_use]
    pub fn related_event(mut self, event_ref: EventRef) -> Self {
        self.related_events.push(event_ref);
        self
    }

    /// Adds a data source.
    #[must_use]
    pub fn data_source(mut self, source: DataSource) -> Self {
        self.data_sources.push(source);
        self
    }

    /// Adds a limitation.
    #[must_use]
    pub fn limitation(mut self, limitation: impl Into<String>) -> Self {
        self.limitations.push(limitation.into());
        self
    }

    /// Builds the diagnosis.
    #[must_use]
    pub fn build(self) -> Diagnosis {
        let summary = self.summary.unwrap_or_else(|| {
            format!(
                "Primary bottleneck: {} ({} confidence, {}%).",
                self.bottleneck_type.label(),
                self.confidence.label(),
                self.confidence.value()
            )
        });

        Diagnosis {
            id: Uuid::new_v4(),
            computed_at: Utc::now(),
            bottleneck_type: self.bottleneck_type,
            confidence: self.confidence,
            summary,
            rules_fired: self.rules_fired,
            contributors: self.contributors,
            time_window: self.time_window.unwrap_or_default(),
            related_events: self.related_events,
            data_sources: self.data_sources,
            limitations: self.limitations,
        }
    }
}

/// A time window for analysis.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TimeWindow {
    /// Start of the window.
    pub start: Option<DateTime<Utc>>,
    /// End of the window.
    pub end: Option<DateTime<Utc>>,
}

impl TimeWindow {
    /// Creates a new time window.
    #[must_use]
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self {
            start: Some(start),
            end: Some(end),
        }
    }

    /// Creates a window for "now" (last N seconds).
    #[must_use]
    pub fn now(duration: Duration) -> Self {
        let end = Utc::now();
        let start = end - chrono::Duration::from_std(duration).unwrap_or_default();
        Self::new(start, end)
    }

    /// Returns the duration of this window.
    #[must_use]
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.start, self.end) {
            (Some(s), Some(e)) => Some(e - s),
            _ => None,
        }
    }
}

/// A rule that matched during diagnosis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleMatch {
    /// Unique rule identifier.
    pub rule_id: String,
    /// Human-readable rule name.
    pub rule_name: String,
    /// Threshold that was exceeded.
    pub threshold: f64,
    /// Measured value.
    pub measured_value: f64,
    /// Unit for the values (e.g., "%", "ms").
    pub unit: String,
    /// Explanation of why this rule fired.
    pub explanation: String,
    /// Evidence references.
    pub evidence: Vec<EvidenceRef>,
}

impl RuleMatch {
    /// Creates a new rule match.
    #[must_use]
    pub fn new(
        rule_id: impl Into<String>,
        rule_name: impl Into<String>,
        threshold: f64,
        measured_value: f64,
        unit: impl Into<String>,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            rule_name: rule_name.into(),
            threshold,
            measured_value,
            unit: unit.into(),
            explanation: explanation.into(),
            evidence: Vec::new(),
        }
    }

    /// Adds evidence to this rule match.
    #[must_use]
    pub fn with_evidence(mut self, evidence: EvidenceRef) -> Self {
        self.evidence.push(evidence);
        self
    }
}

/// Kind of contributor to a bottleneck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContributorKind {
    /// A systemd service/unit.
    Service,
    /// A process (fallback when service mapping fails).
    Process,
    /// A device (disk, network interface).
    Device,
}

impl ContributorKind {
    /// Returns an icon name.
    #[must_use]
    pub const fn icon_name(&self) -> &'static str {
        match self {
            Self::Service => "system-run-symbolic",
            Self::Process => "application-x-executable-symbolic",
            Self::Device => "drive-harddisk-symbolic",
        }
    }
}

/// A contributor to the bottleneck.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    /// Kind of contributor.
    pub kind: ContributorKind,
    /// Name or identifier.
    pub name: String,
    /// Optional PID (for process/service).
    pub pid: Option<u32>,
    /// Optional unit name (for services).
    pub unit_name: Option<String>,
    /// Contribution score (0-100, higher = more responsible).
    pub score: u8,
    /// Trend indicator: positive = increasing, negative = decreasing.
    pub trend: i8,
    /// Evidence supporting this contributor.
    pub evidence: Vec<EvidenceRef>,
}

impl Contributor {
    /// Creates a service contributor.
    #[must_use]
    pub fn service(unit_name: impl Into<String>, score: u8) -> Self {
        let name = unit_name.into();
        Self {
            kind: ContributorKind::Service,
            name: name.clone(),
            pid: None,
            unit_name: Some(name),
            score,
            trend: 0,
            evidence: Vec::new(),
        }
    }

    /// Creates a process contributor.
    #[must_use]
    pub fn process(name: impl Into<String>, pid: u32, score: u8) -> Self {
        Self {
            kind: ContributorKind::Process,
            name: name.into(),
            pid: Some(pid),
            unit_name: None,
            score,
            trend: 0,
            evidence: Vec::new(),
        }
    }

    /// Creates a device contributor.
    #[must_use]
    pub fn device(name: impl Into<String>, score: u8) -> Self {
        Self {
            kind: ContributorKind::Device,
            name: name.into(),
            pid: None,
            unit_name: None,
            score,
            trend: 0,
            evidence: Vec::new(),
        }
    }

    /// Sets the trend.
    #[must_use]
    pub fn with_trend(mut self, trend: i8) -> Self {
        self.trend = trend;
        self
    }

    /// Adds evidence.
    #[must_use]
    pub fn with_evidence(mut self, evidence: EvidenceRef) -> Self {
        self.evidence.push(evidence);
        self
    }
}

/// Reference to evidence supporting a diagnosis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// Type of evidence.
    pub kind: EvidenceKind,
    /// Identifier (sample ID, journal cursor, file path).
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Timestamp if applicable.
    pub timestamp: Option<DateTime<Utc>>,
}

/// Types of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A metrics sample.
    Sample,
    /// A journal log entry.
    JournalEntry,
    /// A procfs reading.
    ProcfsSnapshot,
    /// A timeline event.
    TimelineEvent,
}

/// Reference to a timeline event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRef {
    /// Event ID.
    pub event_id: Uuid,
    /// Brief description.
    pub description: String,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

/// Data sources used for diagnosis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataSource {
    /// /proc/pressure/cpu
    PsiCpu,
    /// /proc/pressure/memory
    PsiMemory,
    /// /proc/pressure/io
    PsiIo,
    /// /proc/stat
    ProcStat,
    /// /proc/meminfo
    ProcMeminfo,
    /// /proc/vmstat
    ProcVmstat,
    /// /proc/diskstats
    ProcDiskstats,
    /// /proc/loadavg
    ProcLoadavg,
    /// /proc/[pid]/stat
    ProcPidStat,
    /// /proc/[pid]/cgroup
    ProcPidCgroup,
    /// journald logs
    Journald,
}

impl DataSource {
    /// Returns a human-readable name.
    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::PsiCpu => "PSI CPU",
            Self::PsiMemory => "PSI Memory",
            Self::PsiIo => "PSI I/O",
            Self::ProcStat => "/proc/stat",
            Self::ProcMeminfo => "/proc/meminfo",
            Self::ProcVmstat => "/proc/vmstat",
            Self::ProcDiskstats => "/proc/diskstats",
            Self::ProcLoadavg => "/proc/loadavg",
            Self::ProcPidStat => "/proc/[pid]/stat",
            Self::ProcPidCgroup => "/proc/[pid]/cgroup",
            Self::Journald => "journald",
        }
    }
}

/// A single pressure sample at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureSample {
    /// Sample ID.
    pub id: Uuid,
    /// When this sample was taken.
    pub timestamp: DateTime<Utc>,
    /// CPU metrics.
    pub cpu: CpuMetrics,
    /// Memory metrics.
    pub memory: MemoryMetrics,
    /// I/O metrics.
    pub io: IoMetrics,
    /// PSI metrics (if available).
    pub psi: Option<PsiMetrics>,
}

/// CPU metrics from a sample.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuMetrics {
    /// CPU utilization percentage (0-100).
    pub utilization: f32,
    /// I/O wait percentage (0-100).
    pub iowait: f32,
    /// System CPU percentage.
    pub system: f32,
    /// User CPU percentage.
    pub user: f32,
    /// 1-minute load average.
    pub load_1m: f32,
    /// 5-minute load average.
    pub load_5m: f32,
    /// Number of runnable processes.
    pub runnable: u32,
}

/// Memory metrics from a sample.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryMetrics {
    /// Total memory in bytes.
    pub total_bytes: u64,
    /// Available memory in bytes.
    pub available_bytes: u64,
    /// Cached memory in bytes.
    pub cached_bytes: u64,
    /// Dirty pages in bytes.
    pub dirty_bytes: u64,
    /// Swap total in bytes.
    pub swap_total_bytes: u64,
    /// Swap free in bytes.
    pub swap_free_bytes: u64,
    /// Pages swapped in since last sample.
    pub pswpin_delta: u64,
    /// Pages swapped out since last sample.
    pub pswpout_delta: u64,
}

impl MemoryMetrics {
    /// Returns memory usage percentage.
    #[must_use]
    pub fn usage_percent(&self) -> f32 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        let used = self.total_bytes.saturating_sub(self.available_bytes);
        (used as f32 / self.total_bytes as f32) * 100.0
    }

    /// Returns swap usage percentage.
    #[must_use]
    pub fn swap_usage_percent(&self) -> f32 {
        if self.swap_total_bytes == 0 {
            return 0.0;
        }
        let used = self.swap_total_bytes.saturating_sub(self.swap_free_bytes);
        (used as f32 / self.swap_total_bytes as f32) * 100.0
    }
}

/// I/O metrics from a sample.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IoMetrics {
    /// Per-device I/O stats.
    pub devices: Vec<DeviceIoMetrics>,
    /// Total read bytes/sec across all devices.
    pub total_read_bytes_sec: u64,
    /// Total write bytes/sec across all devices.
    pub total_write_bytes_sec: u64,
}

/// I/O metrics for a single device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIoMetrics {
    /// Device name (e.g., "sda", "nvme0n1").
    pub name: String,
    /// Read bytes per second.
    pub read_bytes_sec: u64,
    /// Write bytes per second.
    pub write_bytes_sec: u64,
    /// Time spent doing I/O (ms) per second (saturation indicator).
    pub io_time_ms: u64,
    /// Weighted I/O time (queue depth indicator).
    pub weighted_io_time_ms: u64,
}

/// PSI (Pressure Stall Information) metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PsiMetrics {
    /// CPU pressure.
    pub cpu: PsiResource,
    /// Memory pressure.
    pub memory: PsiResourceWithFull,
    /// I/O pressure.
    pub io: PsiResourceWithFull,
}

/// PSI for a resource (some only).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PsiResource {
    /// "some" pressure: percentage of time at least one task was stalled.
    pub some_avg10: f32,
    /// 60-second average of "some" pressure.
    pub some_avg60: f32,
    /// 300-second average of "some" pressure.
    pub some_avg300: f32,
    /// Total stall time in microseconds.
    pub some_total_us: u64,
}

/// PSI for a resource (some + full).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PsiResourceWithFull {
    /// "some" pressure.
    pub some_avg10: f32,
    /// 60-second average of "some" pressure.
    pub some_avg60: f32,
    /// 300-second average of "some" pressure.
    pub some_avg300: f32,
    /// Total "some" stall time in microseconds.
    pub some_total_us: u64,
    /// "full" pressure: percentage of time ALL tasks were stalled.
    pub full_avg10: f32,
    /// 60-second average of "full" pressure.
    pub full_avg60: f32,
    /// 300-second average of "full" pressure.
    pub full_avg300: f32,
    /// Total "full" stall time in microseconds.
    pub full_total_us: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_clamping() {
        assert_eq!(Confidence::new(150).value(), 100);
        assert_eq!(Confidence::new(50).value(), 50);
        assert_eq!(Confidence::new(0).value(), 0);
    }

    #[test]
    fn test_confidence_labels() {
        assert_eq!(Confidence::new(20).label(), "Low");
        assert_eq!(Confidence::new(50).label(), "Medium");
        assert_eq!(Confidence::new(75).label(), "High");
        assert_eq!(Confidence::new(95).label(), "Very High");
    }

    #[test]
    fn test_diagnosis_builder() {
        let diag = Diagnosis::builder(BottleneckType::CpuBound)
            .confidence(85)
            .summary("High CPU usage detected")
            .rule(RuleMatch::new(
                "cpu_high",
                "CPU Utilization High",
                80.0,
                92.5,
                "%",
                "CPU usage exceeded 80% threshold",
            ))
            .contributor(Contributor::service("httpd.service", 75))
            .build();

        assert_eq!(diag.bottleneck_type, BottleneckType::CpuBound);
        assert_eq!(diag.confidence.value(), 85);
        assert_eq!(diag.rules_fired.len(), 1);
        assert_eq!(diag.contributors.len(), 1);
    }

    #[test]
    fn test_memory_metrics_percentages() {
        let metrics = MemoryMetrics {
            total_bytes: 16_000_000_000,
            available_bytes: 4_000_000_000,
            swap_total_bytes: 8_000_000_000,
            swap_free_bytes: 6_000_000_000,
            ..Default::default()
        };

        assert!((metrics.usage_percent() - 75.0).abs() < 0.1);
        assert!((metrics.swap_usage_percent() - 25.0).abs() < 0.1);
    }
}
