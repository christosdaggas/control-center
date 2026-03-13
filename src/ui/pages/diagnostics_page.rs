//! Diagnostics page - System detection and status.

use crate::application::state::SharedState;
use crate::i18n::tr;
use crate::infrastructure::desktop::{
    detect_desktop, detector::DesktopEnvironment,
    portal::detect_capabilities,
};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::debug;

/// The diagnostics page showing system information.
pub struct DiagnosticsPage;

impl DiagnosticsPage {
    /// Creates a new diagnostics page.
    #[must_use]
    pub fn new(state: &SharedState) -> gtk4::Box {
        debug!("Creating diagnostics page");

        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        // Get diagnostics info
        let desktop_info = detect_desktop();
        let portal_caps = detect_capabilities();

        // Create a scrolled window
        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        // Create preferences page
        let prefs_page = adw::PreferencesPage::new();
        prefs_page.set_title(&tr("Diagnostics"));

        // Desktop Environment section
        let desktop_group = adw::PreferencesGroup::new();
        desktop_group.set_title(&tr("Desktop Environment"));
        desktop_group.set_description(Some(&tr("Detected system environment")));

        desktop_group.add(&Self::create_row(
            &tr("Desktop"),
            desktop_info.environment.display_name(),
            Self::status_for_environment(desktop_info.environment),
        ));

        desktop_group.add(&Self::create_row(
            &tr("Session Type"),
            desktop_info.session_type.display_name(),
            StatusLevel::Ok,
        ));

        desktop_group.add(&Self::create_row(
            "XDG_CURRENT_DESKTOP",
            desktop_info.xdg_current_desktop.as_deref().unwrap_or(&tr("Not set")),
            StatusLevel::Neutral,
        ));

        let libadwaita_status = if desktop_info.environment.supports_libadwaita() { tr("Yes") } else { tr("Limited") };
        desktop_group.add(&Self::create_row(
            &tr("libadwaita Compatible"),
            &libadwaita_status,
            if desktop_info.environment.supports_libadwaita() { StatusLevel::Ok } else { StatusLevel::Warning },
        ));

        prefs_page.add(&desktop_group);

        // Portal capabilities section
        let portal_group = adw::PreferencesGroup::new();
        portal_group.set_title(&tr("XDG Desktop Portal"));
        portal_group.set_description(Some(&tr("Portal capabilities for sandboxed operations")));

        let portal_status = if portal_caps.available { tr("Yes") } else { tr("No") };
        portal_group.add(&Self::create_row(
            &tr("Portal Available"),
            &portal_status,
            if portal_caps.available { StatusLevel::Ok } else { StatusLevel::Warning },
        ));

        let file_chooser_status = if portal_caps.file_chooser { tr("Available") } else { tr("Unavailable") };
        portal_group.add(&Self::create_row(
            &tr("File Chooser"),
            &file_chooser_status,
            if portal_caps.file_chooser { StatusLevel::Ok } else { StatusLevel::Neutral },
        ));

        let notifications_status = if portal_caps.notifications { tr("Available") } else { tr("Unavailable") };
        portal_group.add(&Self::create_row(
            &tr("Notifications"),
            &notifications_status,
            if portal_caps.notifications { StatusLevel::Ok } else { StatusLevel::Neutral },
        ));

        prefs_page.add(&portal_group);

        // Event sources section
        let sources_group = adw::PreferencesGroup::new();
        sources_group.set_title(&tr("Event Sources"));
        sources_group.set_description(Some(&tr("Available event data sources")));

        // Check adapter availability
        use crate::infrastructure::adapters::{journald::JournaldAdapter, EventAdapter};
        use crate::infrastructure::adapters::package::{dnf::DnfAdapter, apt::AptAdapter};

        let journald = JournaldAdapter::new();
        let journald_status = if journald.is_available() { tr("Available") } else { tr("Not Available") };
        sources_group.add(&Self::create_row(
            "Journald",
            &journald_status,
            if journald.is_available() { StatusLevel::Ok } else { StatusLevel::Error },
        ));

        let dnf = DnfAdapter::new();
        let dnf_status = if dnf.is_available() { tr("Available") } else { tr("Not Available") };
        sources_group.add(&Self::create_row(
            "DNF (Fedora/RHEL)",
            &dnf_status,
            if dnf.is_available() { StatusLevel::Ok } else { StatusLevel::Neutral },
        ));

        let apt = AptAdapter::new();
        let apt_status = if apt.is_available() { tr("Available") } else { tr("Not Available") };
        sources_group.add(&Self::create_row(
            "APT (Debian/Ubuntu)",
            &apt_status,
            if apt.is_available() { StatusLevel::Ok } else { StatusLevel::Neutral },
        ));

        prefs_page.add(&sources_group);

        // Statistics section (from state)
        let stats_group = adw::PreferencesGroup::new();
        stats_group.set_title(&tr("Statistics"));
        stats_group.set_description(Some(&tr("Current session statistics")));

        if let Ok(state_guard) = state.read() {
            let total_events = state_guard.event_counts.total.to_string();
            stats_group.add(&Self::create_row(
                &tr("Total Events"),
                &total_events,
                StatusLevel::Neutral,
            ));

            if let Some(last) = state_guard.last_ingestion {
                let last_ingestion = last.format("%Y-%m-%d %H:%M:%S").to_string();
                stats_group.add(&Self::create_row(
                    &tr("Last Ingestion"),
                    &last_ingestion,
                    StatusLevel::Neutral,
                ));
            }
        }

        prefs_page.add(&stats_group);

        scrolled.set_child(Some(&prefs_page));
        container.append(&scrolled);

        container
    }

    fn create_row(title: &str, value: &str, status: StatusLevel) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(title)
            .build();

        let label = gtk4::Label::new(Some(value));
        label.add_css_class("diagnostics-value");
        label.add_css_class(status.css_class());
        row.add_suffix(&label);

        row
    }

    fn status_for_environment(env: DesktopEnvironment) -> StatusLevel {
        match env {
            DesktopEnvironment::Gnome | DesktopEnvironment::Cosmic => StatusLevel::Ok,
            DesktopEnvironment::KdePlasma => StatusLevel::Ok,
            DesktopEnvironment::Unknown => StatusLevel::Warning,
            _ => StatusLevel::Neutral,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StatusLevel {
    Ok,
    Warning,
    Error,
    Neutral,
}

impl StatusLevel {
    fn css_class(self) -> &'static str {
        match self {
            Self::Ok => "diagnostics-status-ok",
            Self::Warning => "diagnostics-status-warning",
            Self::Error => "diagnostics-status-error",
            Self::Neutral => "diagnostics-status-neutral",
        }
    }
}
