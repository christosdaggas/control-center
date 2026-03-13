//! Performance Page - Resource Pressure & Bottleneck Analyzer
//!
//! Provides deterministic, rule-based diagnosis of system bottlenecks
//! with evidence chains and top contributors.

use crate::application::use_cases::DiagnosisEngine;
use crate::domain::pressure::{
    BottleneckType, Contributor, ContributorKind, Diagnosis, PressureSample, RuleMatch,
};
use crate::i18n::tr;
use crate::infrastructure::adapters::pressure::{
    create_shared, SamplerCapabilities, SharedRingBuffer, SharedSampler,
};
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Performance analysis page.
pub struct PerformancePage;

impl PerformancePage {
    /// Creates the performance analysis page.
    pub fn new() -> gtk::Box {
        debug!("Creating performance page");

        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.add_css_class("performance-page");
        page.set_hexpand(true);
        page.set_vexpand(true);

        let (sampler, buffer) = create_shared();

        if let Err(e) = sampler.prime() {
            warn!("Failed to prime sampler: {}", e);
        }

        let engine = Arc::new(DiagnosisEngine::new());
        let widgets = Rc::new(RefCell::new(PerformanceWidgets::default()));

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .hexpand(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(16);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);
        content.set_hexpand(true);


        let capability_banner = Self::build_capability_banner(&sampler.capabilities());
        content.append(&capability_banner);
        widgets.borrow_mut().capability_banner = Some(capability_banner);


        let diagnosis_card = Self::build_diagnosis_card(&widgets);
        content.append(&diagnosis_card);


        let gauges = Self::build_pressure_gauges(&widgets);
        content.append(&gauges);


        let evidence_card = Self::build_evidence_card(&widgets);
        content.append(&evidence_card);


        let contributors_card = Self::build_contributors_card(&widgets);
        content.append(&contributors_card);


        let process_drilldown = crate::ui::widgets::create_process_drilldown();
        content.append(&process_drilldown);

        scrolled.set_child(Some(&content));
        page.append(&scrolled);


        Self::refresh_diagnosis(&sampler, &buffer, &engine, &widgets);


        let sampler_clone = sampler.clone();
        let buffer_clone = buffer.clone();
        let engine_clone = engine.clone();
        let widgets_clone = widgets.clone();
        glib::timeout_add_local(Duration::from_secs(2), move || {
            Self::refresh_diagnosis(&sampler_clone, &buffer_clone, &engine_clone, &widgets_clone);
            glib::ControlFlow::Continue
        });

        page
    }

    fn build_capability_banner(caps: &SamplerCapabilities) -> adw::Banner {
        let banner = adw::Banner::new("");

        if !caps.psi.any_available() {
            banner.set_title(&tr("PSI not available - some metrics may be limited (requires kernel 4.20+)"));
            banner.set_revealed(true);
        } else if !caps.psi.all_available() {
            banner.set_title(&tr("Some PSI metrics unavailable"));
            banner.set_revealed(true);
        } else if !caps.has_basic_metrics() {
            banner.set_title(&tr("Limited access to /proc - running in degraded mode"));
            banner.set_revealed(true);
        } else {
            banner.set_revealed(false);
        }

        banner
    }

    fn build_diagnosis_card(widgets: &Rc<RefCell<PerformanceWidgets>>) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 12);
        card.add_css_class("card");
        card.set_margin_bottom(8);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(Some(&tr("Current Diagnosis")));
        title.add_css_class("title-2");
        title.set_halign(gtk::Align::Start);
        header.append(&title);

        let confidence_badge = gtk::Label::new(Some("--"));
        confidence_badge.add_css_class("badge");
        confidence_badge.set_halign(gtk::Align::End);
        confidence_badge.set_hexpand(true);
        header.append(&confidence_badge);
        widgets.borrow_mut().confidence_badge = Some(confidence_badge);

        card.append(&header);

        let type_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let type_icon = gtk::Image::from_icon_name("dialog-information-symbolic");
        type_icon.set_pixel_size(24);
        type_box.append(&type_icon);
        widgets.borrow_mut().type_icon = Some(type_icon);

        let type_label = gtk::Label::new(Some(&tr("Analyzing...")));
        type_label.add_css_class("title-3");
        type_label.set_halign(gtk::Align::Start);
        type_box.append(&type_label);
        widgets.borrow_mut().type_label = Some(type_label);

        card.append(&type_box);

        let summary_label = gtk::Label::new(Some(&tr("Collecting initial samples...")));
        summary_label.add_css_class("heading");
        summary_label.set_halign(gtk::Align::Start);
        summary_label.set_wrap(true);
        summary_label.set_wrap_mode(gtk::pango::WrapMode::Word);
        card.append(&summary_label);
        widgets.borrow_mut().summary_label = Some(summary_label);

        card
    }

    fn build_pressure_gauges(widgets: &Rc<RefCell<PerformanceWidgets>>) -> gtk::Box {
        let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        container.set_homogeneous(true);

        let cpu_gauge = Self::build_gauge(&tr("CPU"), "computer-symbolic");
        container.append(&cpu_gauge.container);
        widgets.borrow_mut().cpu_gauge = Some(cpu_gauge);

        let mem_gauge = Self::build_gauge(&tr("Memory"), "drive-harddisk-symbolic");
        container.append(&mem_gauge.container);
        widgets.borrow_mut().memory_gauge = Some(mem_gauge);

        let io_gauge = Self::build_gauge(&tr("I/O"), "drive-harddisk-symbolic");
        container.append(&io_gauge.container);
        widgets.borrow_mut().io_gauge = Some(io_gauge);

        container
    }

    fn build_gauge(label: &str, icon_name: &str) -> GaugeWidgets {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("card");
        card.set_hexpand(true);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let icon = gtk::Image::from_icon_name(icon_name);
        icon.set_pixel_size(16);
        header.append(&icon);
        let title = gtk::Label::new(Some(label));
        title.add_css_class("caption");
        header.append(&title);
        card.append(&header);

        let value_label = gtk::Label::new(Some("--"));
        value_label.add_css_class("title-1");
        value_label.set_halign(gtk::Align::Start);
        card.append(&value_label);

        let progress = gtk::ProgressBar::new();
        progress.set_fraction(0.0);
        card.append(&progress);

        let psi_label = gtk::Label::new(Some("PSI: --"));
        psi_label.add_css_class("caption");
        psi_label.add_css_class("dim-label");
        psi_label.set_halign(gtk::Align::Start);
        card.append(&psi_label);

        GaugeWidgets {
            container: card,
            value_label,
            progress,
            psi_label,
        }
    }

    fn build_evidence_card(widgets: &Rc<RefCell<PerformanceWidgets>>) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("card");

        let header = gtk::Label::new(Some(&tr("Evidence")));
        header.add_css_class("title-3");
        header.set_halign(gtk::Align::Start);
        card.append(&header);

        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
        card.append(&list);
        widgets.borrow_mut().evidence_list = Some(list);

        let placeholder = gtk::Label::new(Some(&tr("No triggered rules")));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_top(8);
        placeholder.set_margin_bottom(8);
        card.append(&placeholder);
        widgets.borrow_mut().evidence_placeholder = Some(placeholder);

        card
    }

    fn build_contributors_card(widgets: &Rc<RefCell<PerformanceWidgets>>) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("card");

        let header = gtk::Label::new(Some(&tr("Top Contributors")));
        header.add_css_class("title-3");
        header.set_halign(gtk::Align::Start);
        card.append(&header);

        let list = gtk::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_selection_mode(gtk::SelectionMode::None);
        card.append(&list);
        widgets.borrow_mut().contributors_list = Some(list);

        let placeholder = gtk::Label::new(Some(&tr("No significant contributors identified")));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_top(8);
        placeholder.set_margin_bottom(8);
        card.append(&placeholder);
        widgets.borrow_mut().contributors_placeholder = Some(placeholder);

        card
    }

    fn refresh_diagnosis(
        sampler: &SharedSampler,
        buffer: &SharedRingBuffer,
        engine: &Arc<DiagnosisEngine>,
        widgets: &Rc<RefCell<PerformanceWidgets>>,
    ) {
        let sample = match sampler.sample() {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to sample: {}", e);
                return;
            }
        };

        if let Ok(mut buf) = buffer.write() {
            buf.push(sample.clone());
        }

        let diagnosis = engine.diagnose(&sample);
        Self::update_diagnosis_ui(&diagnosis, &sample, widgets);
    }

    fn update_diagnosis_ui(
        diagnosis: &Diagnosis,
        sample: &PressureSample,
        widgets: &Rc<RefCell<PerformanceWidgets>>,
    ) {
        let w = widgets.borrow();


        if let Some(badge) = &w.confidence_badge {
            let conf = &diagnosis.confidence;
            badge.set_label(&format!("{}% {}", conf.value(), conf.label()));
            badge.remove_css_class("success");
            badge.remove_css_class("warning");
            badge.remove_css_class("error");
            badge.add_css_class(match conf.value() {
                0..=30 => "dim-label",
                31..=60 => "warning",
                _ => "error",
            });
        }


        if let Some(icon) = &w.type_icon {
            let (icon_name, css_class) = match &diagnosis.bottleneck_type {
                BottleneckType::NoClearBottleneck => ("checkmark-symbolic", "success"),
                BottleneckType::CpuBound => ("computer-symbolic", "warning"),
                BottleneckType::MemoryPressure => ("drive-harddisk-symbolic", "warning"),
                BottleneckType::IoBound => ("drive-harddisk-symbolic", "warning"),
                BottleneckType::NetworkSuspected => ("network-wired-symbolic", "warning"),
                BottleneckType::MultiFactor => ("dialog-warning-symbolic", "error"),
            };
            icon.set_icon_name(Some(icon_name));
            icon.remove_css_class("success");
            icon.remove_css_class("warning");
            icon.remove_css_class("error");
            icon.add_css_class(css_class);
        }

        if let Some(label) = &w.type_label {
            label.set_label(&diagnosis.bottleneck_type.to_string());
        }

        if let Some(label) = &w.summary_label {
            label.set_label(&diagnosis.summary);
        }


        let psi_cpu = sample.psi.as_ref().map(|p| p.cpu.some_avg10).unwrap_or(0.0);
        let psi_mem = sample.psi.as_ref().map(|p| p.memory.some_avg10).unwrap_or(0.0);
        let psi_io = sample.psi.as_ref().map(|p| p.io.some_avg10).unwrap_or(0.0);

        Self::update_gauge(&w.cpu_gauge, sample.cpu.utilization, psi_cpu);
        Self::update_gauge(&w.memory_gauge, sample.memory.usage_percent(), psi_mem);
        Self::update_gauge(&w.io_gauge, sample.cpu.iowait, psi_io);


        if let Some(list) = &w.evidence_list {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }

            for rule in &diagnosis.rules_fired {
                let row = Self::build_rule_row(rule);
                list.append(&row);
            }
        }

        if let Some(placeholder) = &w.evidence_placeholder {
            placeholder.set_visible(diagnosis.rules_fired.is_empty());
        }


        if let Some(list) = &w.contributors_list {
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }

            for contributor in &diagnosis.contributors {
                let row = Self::build_contributor_row(contributor);
                list.append(&row);
            }
        }

        if let Some(placeholder) = &w.contributors_placeholder {
            placeholder.set_visible(diagnosis.contributors.is_empty());
        }
    }

    fn update_gauge(gauge: &Option<GaugeWidgets>, value: f32, psi: f32) {
        if let Some(g) = gauge {
            g.value_label.set_label(&format!("{:.1}%", value));
            g.progress.set_fraction((value / 100.0).clamp(0.0, 1.0) as f64);
            g.psi_label.set_label(&format!("PSI: {:.1}%", psi));

            g.progress.remove_css_class("success");
            g.progress.remove_css_class("warning");
            g.progress.remove_css_class("error");
            g.progress.add_css_class(match value as u32 {
                0..=60 => "success",
                61..=85 => "warning",
                _ => "error",
            });
        }
    }

    fn build_rule_row(rule: &RuleMatch) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&rule.explanation)
            .subtitle(&format!(
                "Threshold: {:.1}{}, Measured: {:.1}{}",
                rule.threshold, rule.unit, rule.measured_value, rule.unit
            ))
            .build();

        let icon = gtk::Image::from_icon_name("emblem-important-symbolic");
        icon.add_css_class("warning");
        row.add_prefix(&icon);

        row
    }

    fn build_contributor_row(contributor: &Contributor) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(&contributor.name)
            .subtitle(&format!("{:?}", contributor.kind))
            .build();

        let score = gtk::Label::new(Some(&format!("{}%", contributor.score)));
        score.add_css_class("badge");
        if contributor.score >= 70 {
            score.add_css_class("error");
        } else if contributor.score >= 40 {
            score.add_css_class("warning");
        }
        row.add_suffix(&score);

        let icon_name = match contributor.kind {
            ContributorKind::Service => "system-run-symbolic",
            ContributorKind::Process => "application-x-executable-symbolic",
            ContributorKind::Device => "drive-harddisk-symbolic",
        };
        let icon = gtk::Image::from_icon_name(icon_name);
        row.add_prefix(&icon);

        row
    }
}

struct GaugeWidgets {
    container: gtk::Box,
    value_label: gtk::Label,
    progress: gtk::ProgressBar,
    psi_label: gtk::Label,
}

#[derive(Default)]
struct PerformanceWidgets {
    capability_banner: Option<adw::Banner>,
    confidence_badge: Option<gtk::Label>,
    type_icon: Option<gtk::Image>,
    type_label: Option<gtk::Label>,
    summary_label: Option<gtk::Label>,
    cpu_gauge: Option<GaugeWidgets>,
    memory_gauge: Option<GaugeWidgets>,
    io_gauge: Option<GaugeWidgets>,
    evidence_list: Option<gtk::ListBox>,
    evidence_placeholder: Option<gtk::Label>,
    contributors_list: Option<gtk::ListBox>,
    contributors_placeholder: Option<gtk::Label>,
}
