//! Pressure diagnosis use case.
//!
//! Analyzes pressure samples and produces deterministic, rule-based diagnoses
//! with confidence scores and evidence chains.

use crate::domain::pressure::{
    BottleneckType, Contributor, DataSource, Diagnosis, PressureSample, RuleMatch, TimeWindow,
};
use crate::infrastructure::adapters::pressure::UnitMapper;
use std::time::Duration;

/// Thresholds for diagnosis rules.
#[derive(Debug, Clone)]
pub struct DiagnosisThresholds {
    /// CPU usage percentage considered "high".
    pub cpu_high_usage: f32,
    /// CPU usage percentage considered "very high".
    pub cpu_very_high_usage: f32,
    /// Load average per core considered "high".
    pub load_avg_per_core_high: f32,
    /// PSI CPU some stall percentage considered "high".
    pub psi_cpu_some_high: f32,
    /// Memory usage percentage considered "high".
    pub memory_high_percent: f32,
    /// Memory usage percentage considered "critical".
    pub memory_critical_percent: f32,
    /// Swap usage percentage considered "high".
    pub swap_high_percent: f32,
    /// PSI memory some stall percentage considered "high".
    pub psi_memory_some_high: f32,
    /// PSI memory full stall percentage considered "high".
    pub psi_memory_full_high: f32,
    /// PSI I/O some stall percentage considered "high".
    pub psi_io_some_high: f32,
    /// PSI I/O full stall percentage considered "high".
    pub psi_io_full_high: f32,
    /// I/O wait percentage considered "high".
    pub iowait_high: f32,
    /// Number of factors needed to classify as multi-factor.
    pub multi_factor_threshold: u8,
}

impl Default for DiagnosisThresholds {
    fn default() -> Self {
        Self {
            cpu_high_usage: 80.0,
            cpu_very_high_usage: 95.0,
            load_avg_per_core_high: 1.5,
            psi_cpu_some_high: 10.0,
            memory_high_percent: 85.0,
            memory_critical_percent: 95.0,
            swap_high_percent: 50.0,
            psi_memory_some_high: 10.0,
            psi_memory_full_high: 5.0,
            psi_io_some_high: 10.0,
            psi_io_full_high: 5.0,
            iowait_high: 15.0,
            multi_factor_threshold: 2,
        }
    }
}

/// Rule-based diagnosis engine.
pub struct DiagnosisEngine {
    thresholds: DiagnosisThresholds,
}

impl Default for DiagnosisEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosisEngine {
    /// Creates a new diagnosis engine with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            thresholds: DiagnosisThresholds::default(),
        }
    }

    /// Creates a new diagnosis engine with custom thresholds.
    #[must_use]
    pub fn with_thresholds(thresholds: DiagnosisThresholds) -> Self {
        Self { thresholds }
    }

    /// Analyzes a pressure sample and produces a diagnosis with evidence.
    #[must_use]
    pub fn diagnose(&self, sample: &PressureSample) -> Diagnosis {
        let mut rules = Vec::new();
        let mut data_sources = Vec::new();
        let mut factor_count = 0u8;
        let mut confidence_total = 0u8;

        if let Some((cpu_rules, delta)) = self.check_cpu(sample) {
            rules.extend(cpu_rules);
            confidence_total = confidence_total.saturating_add(delta);
            factor_count += 1;
            data_sources.push(DataSource::ProcStat);
        }

        if let Some((mem_rules, delta)) = self.check_memory(sample) {
            rules.extend(mem_rules);
            confidence_total = confidence_total.saturating_add(delta);
            factor_count += 1;
            data_sources.push(DataSource::ProcMeminfo);
        }

        if let Some((io_rules, delta)) = self.check_io(sample) {
            rules.extend(io_rules);
            confidence_total = confidence_total.saturating_add(delta);
            factor_count += 1;
            if sample.psi.is_some() {
                data_sources.push(DataSource::PsiIo);
            }
        }

        let bottleneck_type = self.classify_bottleneck(sample, factor_count);
        let summary = self.generate_summary(&bottleneck_type, sample);
        let contributors = self.get_contributors(&bottleneck_type).unwrap_or_default();

        let mut builder = Diagnosis::builder(bottleneck_type)
            .confidence(confidence_total.min(100))
            .summary(summary)
            .time_window(TimeWindow::now(Duration::from_secs(60)));

        for rule in rules {
            builder = builder.rule(rule);
        }
        for contributor in contributors {
            builder = builder.contributor(contributor);
        }
        for source in data_sources {
            builder = builder.data_source(source);
        }

        builder.build()
    }

    fn check_cpu(&self, sample: &PressureSample) -> Option<(Vec<RuleMatch>, u8)> {
        let mut rules = Vec::new();
        let mut confidence = 0u8;
        let cpu = &sample.cpu;

        if cpu.utilization >= self.thresholds.cpu_very_high_usage {
            rules.push(RuleMatch::new(
                "cpu_very_high", "CPU Very High",
                self.thresholds.cpu_very_high_usage as f64, cpu.utilization as f64, "%",
                format!("CPU usage is very high at {:.1}%", cpu.utilization),
            ));
            confidence += 30;
        } else if cpu.utilization >= self.thresholds.cpu_high_usage {
            rules.push(RuleMatch::new(
                "cpu_high", "CPU High",
                self.thresholds.cpu_high_usage as f64, cpu.utilization as f64, "%",
                format!("CPU usage is high at {:.1}%", cpu.utilization),
            ));
            confidence += 20;
        }

        let core_count = cpu.runnable.max(1);
        let load_per_core = cpu.load_1m / core_count as f32;
        if load_per_core >= self.thresholds.load_avg_per_core_high {
            rules.push(RuleMatch::new(
                "load_high", "Load Average High",
                self.thresholds.load_avg_per_core_high as f64, load_per_core as f64, "",
                format!("Load per core is {:.2}", load_per_core),
            ));
            confidence += 15;
        }

        if let Some(psi) = &sample.psi {
            if psi.cpu.some_avg10 >= self.thresholds.psi_cpu_some_high {
                rules.push(RuleMatch::new(
                    "psi_cpu_high", "PSI CPU Pressure",
                    self.thresholds.psi_cpu_some_high as f64, psi.cpu.some_avg10 as f64, "%",
                    format!("CPU pressure (PSI) at {:.1}%", psi.cpu.some_avg10),
                ));
                confidence += 25;
            }
        }

        if rules.is_empty() { None } else { Some((rules, confidence.min(50))) }
    }

    fn check_memory(&self, sample: &PressureSample) -> Option<(Vec<RuleMatch>, u8)> {
        let mut rules = Vec::new();
        let mut confidence = 0u8;
        let mem = &sample.memory;
        let usage_pct = mem.usage_percent();

        if usage_pct >= self.thresholds.memory_critical_percent {
            rules.push(RuleMatch::new(
                "memory_critical", "Memory Critical",
                self.thresholds.memory_critical_percent as f64, usage_pct as f64, "%",
                format!("Memory usage is critical at {:.1}%", usage_pct),
            ));
            confidence += 35;
        } else if usage_pct >= self.thresholds.memory_high_percent {
            rules.push(RuleMatch::new(
                "memory_high", "Memory High",
                self.thresholds.memory_high_percent as f64, usage_pct as f64, "%",
                format!("Memory usage is high at {:.1}%", usage_pct),
            ));
            confidence += 20;
        }

        let swap_pct = mem.swap_usage_percent();
        if swap_pct >= self.thresholds.swap_high_percent {
            rules.push(RuleMatch::new(
                "swap_high", "Swap Usage High",
                self.thresholds.swap_high_percent as f64, swap_pct as f64, "%",
                format!("Swap usage at {:.1}%", swap_pct),
            ));
            confidence += 15;
        }

        if let Some(psi) = &sample.psi {
            if psi.memory.full_avg10 >= self.thresholds.psi_memory_full_high {
                rules.push(RuleMatch::new(
                    "psi_memory_full", "Memory Stall (full)",
                    self.thresholds.psi_memory_full_high as f64, psi.memory.full_avg10 as f64, "%",
                    format!("Memory full stall at {:.1}%", psi.memory.full_avg10),
                ));
                confidence += 30;
            } else if psi.memory.some_avg10 >= self.thresholds.psi_memory_some_high {
                rules.push(RuleMatch::new(
                    "psi_memory_some", "Memory Stall (some)",
                    self.thresholds.psi_memory_some_high as f64, psi.memory.some_avg10 as f64, "%",
                    format!("Memory some stall at {:.1}%", psi.memory.some_avg10),
                ));
                confidence += 20;
            }
        }

        if rules.is_empty() { None } else { Some((rules, confidence.min(50))) }
    }

    fn check_io(&self, sample: &PressureSample) -> Option<(Vec<RuleMatch>, u8)> {
        let mut rules = Vec::new();
        let mut confidence = 0u8;

        if sample.cpu.iowait >= self.thresholds.iowait_high {
            rules.push(RuleMatch::new(
                "iowait_high", "I/O Wait High",
                self.thresholds.iowait_high as f64, sample.cpu.iowait as f64, "%",
                format!("CPU iowait at {:.1}%", sample.cpu.iowait),
            ));
            confidence += 20;
        }

        if let Some(psi) = &sample.psi {
            if psi.io.full_avg10 >= self.thresholds.psi_io_full_high {
                rules.push(RuleMatch::new(
                    "psi_io_full", "I/O Stall (full)",
                    self.thresholds.psi_io_full_high as f64, psi.io.full_avg10 as f64, "%",
                    format!("I/O full stall at {:.1}%", psi.io.full_avg10),
                ));
                confidence += 30;
            } else if psi.io.some_avg10 >= self.thresholds.psi_io_some_high {
                rules.push(RuleMatch::new(
                    "psi_io_some", "I/O Stall (some)",
                    self.thresholds.psi_io_some_high as f64, psi.io.some_avg10 as f64, "%",
                    format!("I/O some stall at {:.1}%", psi.io.some_avg10),
                ));
                confidence += 20;
            }
        }

        if rules.is_empty() { None } else { Some((rules, confidence.min(50))) }
    }

    fn classify_bottleneck(&self, sample: &PressureSample, factor_count: u8) -> BottleneckType {
        if factor_count >= self.thresholds.multi_factor_threshold {
            return BottleneckType::MultiFactor;
        }

        let psi = sample.psi.as_ref();

        let cpu_pressure = sample.cpu.utilization >= self.thresholds.cpu_high_usage
            || psi.map(|p| p.cpu.some_avg10 >= self.thresholds.psi_cpu_some_high).unwrap_or(false);

        let memory_pressure = sample.memory.usage_percent() >= self.thresholds.memory_high_percent
            || psi.map(|p| p.memory.some_avg10 >= self.thresholds.psi_memory_some_high).unwrap_or(false);

        let io_pressure = sample.cpu.iowait >= self.thresholds.iowait_high
            || psi.map(|p| p.io.some_avg10 >= self.thresholds.psi_io_some_high).unwrap_or(false);

        match (cpu_pressure, memory_pressure, io_pressure) {
            (true, false, false) => BottleneckType::CpuBound,
            (false, true, false) => BottleneckType::MemoryPressure,
            (false, false, true) => BottleneckType::IoBound,
            (true, true, _) | (true, _, true) | (_, true, true) => BottleneckType::MultiFactor,
            (false, false, false) => BottleneckType::NoClearBottleneck,
        }
    }

    fn generate_summary(&self, bottleneck: &BottleneckType, sample: &PressureSample) -> String {
        match bottleneck {
            BottleneckType::CpuBound => {
                format!("CPU-bound: {:.0}% usage, load {:.1}", sample.cpu.utilization, sample.cpu.load_1m)
            }
            BottleneckType::MemoryPressure => {
                format!("Memory pressure: {:.0}% used ({} available)",
                    sample.memory.usage_percent(), format_bytes(sample.memory.available_bytes))
            }
            BottleneckType::IoBound => {
                format!("I/O-bound: {:.0}% iowait", sample.cpu.iowait)
            }
            BottleneckType::NetworkSuspected => "Network issues suspected".to_string(),
            BottleneckType::MultiFactor => "Multiple bottlenecks detected".to_string(),
            BottleneckType::NoClearBottleneck => "System operating normally".to_string(),
        }
    }

    fn get_contributors(&self, bottleneck: &BottleneckType) -> Result<Vec<Contributor>, String> {
        let units = match bottleneck {
            BottleneckType::CpuBound | BottleneckType::MemoryPressure | BottleneckType::MultiFactor => {
                UnitMapper::top_by_memory(5).map_err(|e| e.to_string())?
            }
            BottleneckType::IoBound => {
                UnitMapper::top_by_io(5).map_err(|e| e.to_string())?
            }
            _ => return Ok(Vec::new()),
        };

        Ok(units
            .into_iter()
            .filter_map(|unit_stats| {
                let name = unit_stats.unit.as_ref()
                    .map(|u| u.display_name())
                    .unwrap_or_else(|| "kernel".to_string());

                if name == "kernel" && unit_stats.process_count < 5 {
                    return None;
                }

                let score = match bottleneck {
                    BottleneckType::MemoryPressure => {
                        (unit_stats.rss_bytes as f32 / (1024.0 * 1024.0 * 1024.0) * 100.0).min(100.0) as u8
                    }
                    BottleneckType::IoBound => {
                        let io_mb = (unit_stats.read_bytes + unit_stats.write_bytes) as f32 / (1024.0 * 1024.0);
                        (io_mb.log10() * 20.0).clamp(0.0, 100.0) as u8
                    }
                    _ => 50,
                };

                let is_user = unit_stats.unit.as_ref().map(|u| u.is_user).unwrap_or(false);
                if is_user {
                    Some(Contributor::process(name, 0, score))
                } else {
                    Some(Contributor::service(name, score))
                }
            })
            .take(5)
            .collect())
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pressure::{CpuMetrics, IoMetrics, MemoryMetrics};
    use uuid::Uuid;
    use chrono::Utc;

    fn make_sample() -> PressureSample {
        PressureSample {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            cpu: CpuMetrics::default(),
            memory: MemoryMetrics::default(),
            io: IoMetrics::default(),
            psi: None,
        }
    }

    #[test]
    fn test_no_bottleneck() {
        let engine = DiagnosisEngine::new();
        let sample = make_sample();
        let diagnosis = engine.diagnose(&sample);
        assert_eq!(diagnosis.bottleneck_type, BottleneckType::NoClearBottleneck);
    }

    #[test]
    fn test_cpu_bound() {
        let engine = DiagnosisEngine::new();
        let mut sample = make_sample();
        sample.cpu.utilization = 95.0;
        sample.cpu.load_1m = 8.0;
        sample.cpu.runnable = 4;
        
        let diagnosis = engine.diagnose(&sample);
        assert_eq!(diagnosis.bottleneck_type, BottleneckType::CpuBound);
        assert!(!diagnosis.rules_fired.is_empty());
    }
}
