//! System Health Page - Modern Dashboard Design
//!
//! Displays system status with a KPI strip at top and detail cards below.

use crate::i18n::tr;
use crate::infrastructure::adapters::{MemoryStats, SystemStatsAdapter, SystemdAdapter};
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};
use std::cell::RefCell;
use std::rc::Rc;
use tracing::debug;

/// Data passed from background thread to UI.
struct HealthData {
    health: crate::infrastructure::adapters::SystemHealth,
    running_count: usize,
    total_failed: usize,
    user_running: usize,
}

/// System health page showing quick status overview.
pub struct SystemHealthPage;

impl SystemHealthPage {
    /// Creates the system health page.
    pub fn new() -> gtk::Box {
        debug!("Creating system health page");

        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.add_css_class("dashboard-page");


        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();


        let content = gtk::Box::new(gtk::Orientation::Vertical, 20);
        content.set_margin_top(16);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);


        let widgets = Rc::new(RefCell::new(HealthWidgets::default()));

        // === HEADER CHIPS (Hostname + OS) ===
        let header_chips = Self::build_header_chips(&widgets);
        content.append(&header_chips);


        let kpi_strip = Self::build_kpi_strip(&widgets);
        content.append(&kpi_strip);


        let details_grid = Self::build_details_grid(&widgets);
        content.append(&details_grid);

        scrolled.set_child(Some(&content));
        page.append(&scrolled);


        // Initial async refresh
        Self::refresh_data_async(&widgets);

        // Periodic refresh every 5 seconds
        let widgets_clone = widgets.clone();
        glib::timeout_add_seconds_local(5, move || {
            Self::refresh_data_async(&widgets_clone);
            glib::ControlFlow::Continue
        });

        page
    }

    /// Spawns a background thread to collect health data, then updates UI on main thread.
    fn refresh_data_async(widgets: &Rc<RefCell<HealthWidgets>>) {
        debug!("Refreshing system health data (async)");

        let (tx, rx) = std::sync::mpsc::channel::<HealthData>();

        std::thread::spawn(move || {
            let health = SystemStatsAdapter::read_system_health();

            let system_units = SystemdAdapter::list_units(
                Some(crate::infrastructure::adapters::UnitType::Service),
                false,
            );
            let user_units = SystemdAdapter::list_units(
                Some(crate::infrastructure::adapters::UnitType::Service),
                true,
            );

            let running_count = system_units
                .iter()
                .filter(|u| u.state == crate::infrastructure::adapters::UnitState::Active)
                .count();
            let failed_system = SystemdAdapter::failed_count(false);
            let failed_user = SystemdAdapter::failed_count(true);
            let total_failed = failed_system + failed_user;
            let user_running = user_units
                .iter()
                .filter(|u| u.state == crate::infrastructure::adapters::UnitState::Active)
                .count();

            let _ = tx.send(HealthData {
                health,
                running_count,
                total_failed,
                user_running,
            });
        });

        // Poll the channel from the main thread
        let widgets = widgets.clone();
        glib::idle_add_local(move || {
            match rx.try_recv() {
                Ok(data) => {
                    Self::update_ui(&widgets, data);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    }


    fn build_header_chips(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let chips_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        chips_box.set_margin_bottom(4);


        let hostname_chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        hostname_chip.add_css_class("chip");
        let hostname_icon = gtk::Image::from_icon_name("computer-symbolic");
        hostname_icon.set_pixel_size(14);
        hostname_chip.append(&hostname_icon);
        let hostname_label = gtk::Label::new(Some(&tr("Loading...")));
        hostname_label.add_css_class("chip-label");
        hostname_chip.append(&hostname_label);
        chips_box.append(&hostname_chip);
        widgets.borrow_mut().hostname_label = Some(hostname_label);


        let os_chip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        os_chip.add_css_class("chip");
        let os_icon = gtk::Image::from_icon_name("emblem-system-symbolic");
        os_icon.set_pixel_size(14);
        os_chip.append(&os_icon);
        let os_label = gtk::Label::new(Some(&tr("Loading...")));
        os_label.add_css_class("chip-label");
        os_chip.append(&os_label);
        chips_box.append(&os_chip);
        widgets.borrow_mut().os_label = Some(os_label);

        chips_box
    }

    /// Builds the KPI strip with hero metrics.
    fn build_kpi_strip(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::FlowBox {
        let flow_box = gtk::FlowBox::new();
        flow_box.set_homogeneous(true);
        flow_box.set_min_children_per_line(5);
        flow_box.set_max_children_per_line(5);
        flow_box.set_selection_mode(gtk::SelectionMode::None);
        flow_box.set_row_spacing(8);
        flow_box.set_column_spacing(10);
        flow_box.add_css_class("kpi-strip");


        let cpu_kpi = Self::build_kpi_tile(
            &tr("CPU"),
            "0%",
            Some("computer-symbolic"),
            true,
            &widgets,
            |w, tile| w.cpu_kpi_tile = Some(tile),
        );
        flow_box.insert(&cpu_kpi, -1);


        let ram_kpi = Self::build_kpi_tile(
            &tr("RAM"),
            "0 GB / 0 GB",
            Some("drive-harddisk-symbolic"),
            true,
            &widgets,
            |w, tile| w.ram_kpi_tile = Some(tile),
        );
        flow_box.insert(&ram_kpi, -1);


        let swap_kpi = Self::build_kpi_tile(
            &tr("Swap"),
            "0 KB / 0 GB",
            Some("drive-multidisk-symbolic"),
            true,
            &widgets,
            |w, tile| w.swap_kpi_tile = Some(tile),
        );
        flow_box.insert(&swap_kpi, -1);


        let uptime_kpi = Self::build_kpi_tile(
            &tr("Uptime"),
            "0h 0m",
            Some("preferences-system-time-symbolic"),
            false,
            &widgets,
            |w, tile| w.uptime_kpi_tile = Some(tile),
        );
        flow_box.insert(&uptime_kpi, -1);


        let services_kpi = Self::build_services_kpi_tile(&widgets);
        flow_box.insert(&services_kpi, -1);

        flow_box
    }

    /// Builds a single KPI tile.
    fn build_kpi_tile<F>(
        title: &str,
        initial_value: &str,
        icon_name: Option<&str>,
        with_progress: bool,
        _widgets: &Rc<RefCell<HealthWidgets>>,
        mut store_fn: F,
    ) -> gtk::Box
    where
        F: FnMut(&mut HealthWidgets, KpiTile),
    {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
        tile.add_css_class("kpi-tile");
        tile.set_hexpand(true);


        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        if let Some(icon) = icon_name {
            let icon_widget = gtk::Image::from_icon_name(icon);
            icon_widget.add_css_class("kpi-icon");
            icon_widget.set_pixel_size(16);
            header.append(&icon_widget);
        }
        let title_label = gtk::Label::new(Some(title));
        title_label.add_css_class("kpi-title");
        header.append(&title_label);
        tile.append(&header);


        let value_label = gtk::Label::new(Some(initial_value));
        value_label.add_css_class("kpi-value");
        value_label.set_halign(gtk::Align::Start);
        tile.append(&value_label);


        let progress = if with_progress {
            let bar = gtk::LevelBar::new();
            bar.set_min_value(0.0);
            bar.set_max_value(100.0);
            bar.set_value(0.0);
            bar.add_css_class("kpi-progress");
            tile.append(&bar);
            Some(bar)
        } else {
            None
        };

        let kpi_tile = KpiTile {
            value_label,
            progress,
        };
        store_fn(&mut _widgets.borrow_mut(), kpi_tile);

        tile
    }

    /// Builds the Services KPI tile with running/failed counts.
    fn build_services_kpi_tile(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
        tile.add_css_class("kpi-tile");
        tile.set_hexpand(true);


        let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let icon = gtk::Image::from_icon_name("system-run-symbolic");
        icon.add_css_class("kpi-icon");
        icon.set_pixel_size(16);
        header.append(&icon);
        let title_label = gtk::Label::new(Some(&tr("Services")));
        title_label.add_css_class("kpi-title");
        header.append(&title_label);
        tile.append(&header);


        let value_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        value_row.set_halign(gtk::Align::Start);

        let running_label = gtk::Label::new(Some("0"));
        running_label.add_css_class("kpi-value");
        value_row.append(&running_label);

        let running_text = gtk::Label::new(Some(&tr("running")));
        running_text.add_css_class("kpi-unit");
        value_row.append(&running_text);


        let failed_badge = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        failed_badge.add_css_class("failed-badge");
        let failed_label = gtk::Label::new(Some("0"));
        failed_label.add_css_class("failed-count");
        failed_badge.append(&failed_label);
        let failed_text = gtk::Label::new(Some(&tr("failed")));
        failed_text.add_css_class("failed-text");
        failed_badge.append(&failed_text);
        value_row.append(&failed_badge);

        tile.append(&value_row);

        widgets.borrow_mut().services_running_kpi = Some(running_label);
        widgets.borrow_mut().services_failed_kpi = Some(failed_label);
        widgets.borrow_mut().services_failed_badge = Some(failed_badge);

        tile
    }

    /// Builds the 2-column detail cards grid.
    fn build_details_grid(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let grid = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        grid.set_homogeneous(true);


        let left_col = gtk::Box::new(gtk::Orientation::Vertical, 16);


        let cpu_card = Self::build_cpu_card(widgets);
        left_col.append(&cpu_card);


        let services_card = Self::build_services_detail_card(widgets);
        left_col.append(&services_card);

        grid.append(&left_col);


        let right_col = gtk::Box::new(gtk::Orientation::Vertical, 16);


        let system_card = Self::build_system_info_card(widgets);
        right_col.append(&system_card);


        let disk_card = Self::build_disk_card(widgets);
        right_col.append(&disk_card);

        grid.append(&right_col);

        grid
    }

    /// Builds the CPU & Compute detail card.
    fn build_cpu_card(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let card = Self::create_card(&tr("CPU & Compute"));


        let usage_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        usage_row.set_margin_bottom(12);

        let usage_icon = gtk::Image::from_icon_name("emblem-system-symbolic");
        usage_icon.set_pixel_size(16);
        usage_icon.add_css_class("dim-label");
        usage_row.append(&usage_icon);

        let usage_label = gtk::Label::new(Some(&tr("Usage")));
        usage_label.add_css_class("card-row-label");
        usage_label.set_hexpand(true);
        usage_label.set_halign(gtk::Align::Start);
        usage_row.append(&usage_label);

        let cpu_progress = gtk::LevelBar::new();
        cpu_progress.set_min_value(0.0);
        cpu_progress.set_max_value(100.0);
        cpu_progress.set_value(0.0);
        cpu_progress.set_width_request(180);
        cpu_progress.set_valign(gtk::Align::Center);
        usage_row.append(&cpu_progress);

        let cpu_percent = gtk::Label::new(Some("0%"));
        cpu_percent.add_css_class("card-value");
        cpu_percent.set_width_chars(4);
        usage_row.append(&cpu_percent);

        card.append(&usage_row);
        widgets.borrow_mut().cpu_progress = Some(cpu_progress);
        widgets.borrow_mut().cpu_percent_label = Some(cpu_percent);


        let proc_row = Self::create_info_row(&tr("Processor"), &tr("Loading..."));
        card.append(&proc_row.0);
        widgets.borrow_mut().cpu_model_label = Some(proc_row.1);


        let cores_row = Self::create_info_row(&tr("CPU Cores"), "0");
        card.append(&cores_row.0);
        widgets.borrow_mut().cpu_cores_label = Some(cores_row.1);


        let load_row = Self::create_info_row(&tr("Load Average"), "0 / 0 / 0");
        card.append(&load_row.0);
        widgets.borrow_mut().load_label = Some(load_row.1);

        card
    }

    /// Builds the Services Status detail card.
    fn build_services_detail_card(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let card = Self::create_card(&tr("Services Status"));


        let running_row = Self::create_status_row(
            &tr("Running Services"),
            "0",
            "status-running",
        );
        card.append(&running_row.0);
        widgets.borrow_mut().running_services_label = Some(running_row.1);


        let failed_row = Self::create_status_row(
            &tr("Failed Services"),
            "0",
            "status-failed",
        );
        card.append(&failed_row.0);
        widgets.borrow_mut().failed_services_label = Some(failed_row.1);


        let user_row = Self::create_status_row(
            &tr("User Services"),
            "0",
            "status-neutral",
        );
        card.append(&user_row.0);
        widgets.borrow_mut().user_services_label = Some(user_row.1);

        card
    }

    /// Builds the System Information detail card.
    fn build_system_info_card(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let card = Self::create_card(&tr("System Information"));


        let hostname_row = Self::create_info_row(&tr("Hostname"), &tr("Loading..."));
        card.append(&hostname_row.0);
        widgets.borrow_mut().hostname_detail_label = Some(hostname_row.1);


        let os_row = Self::create_info_row(&tr("Operating System"), &tr("Loading..."));
        card.append(&os_row.0);
        widgets.borrow_mut().os_detail_label = Some(os_row.1);


        let kernel_row = Self::create_info_row(&tr("Kernel Version"), &tr("Loading..."));
        card.append(&kernel_row.0);
        widgets.borrow_mut().kernel_label = Some(kernel_row.1);


        let uptime_row = Self::create_info_row(&tr("Uptime"), &tr("Loading..."));
        card.append(&uptime_row.0);
        widgets.borrow_mut().uptime_label = Some(uptime_row.1);

        card
    }

    /// Builds the Disk Usage detail card.
    fn build_disk_card(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let card = Self::create_card(&tr("Disk Usage"));


        let disk_container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        disk_container.add_css_class("disk-container");


        let loading_label = gtk::Label::new(Some(&tr("Loading disk information...")));
        loading_label.add_css_class("dim-label");
        disk_container.append(&loading_label);

        card.append(&disk_container);
        widgets.borrow_mut().disk_container = Some(disk_container);

        card
    }

    /// Creates a card container with title.
    fn create_card(title: &str) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 8);
        card.add_css_class("detail-card");

        let title_label = gtk::Label::new(Some(title));
        title_label.add_css_class("card-title");
        title_label.set_halign(gtk::Align::Start);
        card.append(&title_label);

        card
    }

    /// Creates an info row with label and value.
    fn create_info_row(label: &str, initial_value: &str) -> (gtk::Box, gtk::Label) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("info-row");

        let label_widget = gtk::Label::new(Some(label));
        label_widget.add_css_class("info-label");
        label_widget.set_halign(gtk::Align::Start);
        label_widget.set_hexpand(true);
        row.append(&label_widget);

        let value_widget = gtk::Label::new(Some(initial_value));
        value_widget.add_css_class("info-value");
        value_widget.set_halign(gtk::Align::End);
        value_widget.set_ellipsize(gtk::pango::EllipsizeMode::End);
        value_widget.set_max_width_chars(35);
        row.append(&value_widget);

        (row, value_widget)
    }

    /// Creates a status row with icon, label, and count.
    fn create_status_row(
        label: &str,
        initial_value: &str,
        css_class: &str,
    ) -> (gtk::Box, gtk::Label) {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.add_css_class("status-row");


        let dot = gtk::DrawingArea::new();
        dot.set_content_width(8);
        dot.set_content_height(8);
        dot.set_valign(gtk::Align::Center);
        dot.add_css_class("status-dot");
        dot.add_css_class(css_class);
        row.append(&dot);

        let label_widget = gtk::Label::new(Some(label));
        label_widget.add_css_class("status-label");
        label_widget.set_halign(gtk::Align::Start);
        label_widget.set_hexpand(true);
        row.append(&label_widget);

        let value_widget = gtk::Label::new(Some(initial_value));
        value_widget.add_css_class("status-value");
        value_widget.add_css_class(css_class);
        value_widget.set_halign(gtk::Align::End);
        row.append(&value_widget);

        (row, value_widget)
    }

    /// Creates an empty state widget for disk card.
    fn create_disk_empty_state(widgets: &Rc<RefCell<HealthWidgets>>) -> gtk::Box {
        let container = gtk::Box::new(gtk::Orientation::Vertical, 8);
        container.add_css_class("empty-state");
        container.set_valign(gtk::Align::Center);

        let title = gtk::Label::new(Some(&tr("No disks detected")));
        title.add_css_class("empty-state-title");
        container.append(&title);

        let subtitle = gtk::Label::new(Some(&tr("Disk metrics provider returned no devices.")));
        subtitle.add_css_class("empty-state-subtitle");
        container.append(&subtitle);

        let rescan_btn = gtk::Button::with_label(&tr("Rescan"));
        rescan_btn.add_css_class("pill");
        rescan_btn.set_halign(gtk::Align::Center);
        rescan_btn.set_margin_top(8);
        

        let widgets_clone = widgets.clone();
        rescan_btn.connect_clicked(move |_| {
            Self::refresh_data_async(&widgets_clone);
        });
        
        container.append(&rescan_btn);

        container
    }

    /// Creates a disk row with progress bar.
    fn create_disk_row(
        mount: &str,
        device: &str,
        fs_type: &str,
        used: u64,
        total: u64,
        percent: f64,
    ) -> gtk::Box {
        let row = gtk::Box::new(gtk::Orientation::Vertical, 2);
        row.add_css_class("disk-row");


        let progress = gtk::LevelBar::new();
        progress.set_min_value(0.0);
        progress.set_max_value(100.0);
        progress.set_value(percent);
        progress.add_css_class("disk-progress");
        if percent > 90.0 {
            progress.add_css_class("disk-critical");
        } else if percent > 75.0 {
            progress.add_css_class("disk-warning");
        }
        row.append(&progress);


        let info_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);

        let mount_label = gtk::Label::new(Some(mount));
        mount_label.add_css_class("disk-mount");
        mount_label.set_halign(gtk::Align::Start);
        info_row.append(&mount_label);

        let usage_label = gtk::Label::new(Some(&format!(
            "{} / {}",
            MemoryStats::format_bytes(used),
            MemoryStats::format_bytes(total)
        )));
        usage_label.add_css_class("disk-usage");
        usage_label.set_halign(gtk::Align::Start);
        usage_label.set_hexpand(true);
        info_row.append(&usage_label);

        let device_label = gtk::Label::new(Some(&format!("{} ({})", device, fs_type)));
        device_label.add_css_class("disk-subtitle");
        device_label.set_halign(gtk::Align::End);
        info_row.append(&device_label);

        row.append(&info_row);

        row
    }




    /// Updates the UI with fresh data (runs on main thread).
    fn update_ui(widgets: &Rc<RefCell<HealthWidgets>>, data: HealthData) {
        let health = data.health;
        let running_count = data.running_count;
        let total_failed = data.total_failed;
        let user_running = data.user_running;

        let w = widgets.borrow();


        if let Some(label) = &w.hostname_label {
            label.set_label(&health.hostname);
        }
        if let Some(label) = &w.os_label {
            label.set_label(&health.os_name);
        }


        if let Some(tile) = &w.cpu_kpi_tile {
            tile.value_label
                .set_label(&format!("{:.0}%", health.cpu.usage_percent));
            if let Some(bar) = &tile.progress {
                bar.set_value(health.cpu.usage_percent as f64);
            }
        }
        if let Some(tile) = &w.ram_kpi_tile {
            tile.value_label.set_label(&format!(
                "{} / {}",
                MemoryStats::format_bytes(health.memory.used_bytes),
                MemoryStats::format_bytes(health.memory.total_bytes)
            ));
            if let Some(bar) = &tile.progress {
                bar.set_value(health.memory.usage_percent() as f64);
            }
        }
        if let Some(tile) = &w.swap_kpi_tile {
            tile.value_label.set_label(&format!(
                "{} / {}",
                MemoryStats::format_bytes(health.memory.swap_used_bytes),
                MemoryStats::format_bytes(health.memory.swap_total_bytes)
            ));
            if let Some(bar) = &tile.progress {
                bar.set_value(health.memory.swap_usage_percent() as f64);
            }
        }
        if let Some(tile) = &w.uptime_kpi_tile {

            let hours = (health.uptime.uptime_secs % 86400) / 3600;
            let mins = (health.uptime.uptime_secs % 3600) / 60;
            tile.value_label.set_label(&format!("{}h {}m", hours, mins));
        }


        if let Some(label) = &w.services_running_kpi {
            label.set_label(&format!("{}", running_count));
        }
        if let Some(label) = &w.services_failed_kpi {
            label.set_label(&format!("{}", total_failed));
        }
        if let Some(badge) = &w.services_failed_badge {
            if total_failed > 0 {
                badge.add_css_class("has-failures");
            } else {
                badge.remove_css_class("has-failures");
            }
        }


        if let Some(progress) = &w.cpu_progress {
            progress.set_value(health.cpu.usage_percent as f64);
        }
        if let Some(label) = &w.cpu_percent_label {
            label.set_label(&format!("{:.0}%", health.cpu.usage_percent));
        }
        if let Some(label) = &w.cpu_model_label {
            let model = if health.cpu.model_name.is_empty() {
                tr("Unknown")
            } else {
                health.cpu.model_name.clone()
            };
            label.set_label(&model);
            label.set_tooltip_text(Some(&model));
        }
        if let Some(label) = &w.cpu_cores_label {
            label.set_label(&format!("{}", health.cpu.core_count));
        }
        if let Some(label) = &w.load_label {
            label.set_label(&format!(
                "{:.2} / {:.2} / {:.2}",
                health.load.one_min, health.load.five_min, health.load.fifteen_min
            ));
        }


        if let Some(label) = &w.hostname_detail_label {
            label.set_label(&health.hostname);
        }
        if let Some(label) = &w.os_detail_label {
            label.set_label(&health.os_name);
        }
        if let Some(label) = &w.kernel_label {
            label.set_label(&health.kernel_version);
        }
        if let Some(label) = &w.uptime_label {
            label.set_label(&health.uptime.format());
        }


        if let Some(label) = &w.running_services_label {
            label.set_label(&format!("{}", running_count));
        }
        if let Some(label) = &w.failed_services_label {
            label.set_label(&format!("{}", total_failed));
            if total_failed > 0 {
                label.add_css_class("error");
            } else {
                label.remove_css_class("error");
            }
        }
        if let Some(label) = &w.user_services_label {
            label.set_label(&format!("{}", user_running));
        }



        let disk_container = w.disk_container.clone();
        drop(w); // Drop the borrow so we can pass widgets to create_disk_empty_state

        if let Some(container) = disk_container {

            while let Some(child) = container.first_child() {
                container.remove(&child);
            }

            if health.disks.is_empty() {
                let empty_state = Self::create_disk_empty_state(widgets);
                container.append(&empty_state);
            } else {
                for disk in &health.disks {
                    let row = Self::create_disk_row(
                        &disk.mount_point,
                        &disk.device,
                        &disk.fs_type,
                        disk.used_bytes,
                        disk.total_bytes,
                        disk.usage_percent() as f64,
                    );
                    container.append(&row);
                }
            }
        }
    }
}

/// KPI tile widget references.
struct KpiTile {
    value_label: gtk::Label,
    progress: Option<gtk::LevelBar>,
}

/// Container for widget references that need updating.
#[derive(Default)]
struct HealthWidgets {
    // Header chips
    hostname_label: Option<gtk::Label>,
    os_label: Option<gtk::Label>,

    // KPI tiles
    cpu_kpi_tile: Option<KpiTile>,
    ram_kpi_tile: Option<KpiTile>,
    swap_kpi_tile: Option<KpiTile>,
    uptime_kpi_tile: Option<KpiTile>,
    services_running_kpi: Option<gtk::Label>,
    services_failed_kpi: Option<gtk::Label>,
    services_failed_badge: Option<gtk::Box>,

    // CPU detail card
    cpu_progress: Option<gtk::LevelBar>,
    cpu_percent_label: Option<gtk::Label>,
    cpu_model_label: Option<gtk::Label>,
    cpu_cores_label: Option<gtk::Label>,
    load_label: Option<gtk::Label>,

    // System info detail card
    hostname_detail_label: Option<gtk::Label>,
    os_detail_label: Option<gtk::Label>,
    kernel_label: Option<gtk::Label>,
    uptime_label: Option<gtk::Label>,

    // Services detail card
    running_services_label: Option<gtk::Label>,
    failed_services_label: Option<gtk::Label>,
    user_services_label: Option<gtk::Label>,

    // Disk card
    disk_container: Option<gtk::Box>,
}
