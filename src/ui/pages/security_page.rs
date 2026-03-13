//! Security posture page.

use crate::domain::event::Severity;
use crate::i18n::tr;
use crate::infrastructure::adapters::{SecurityAdapter, SecurityFinding, SecurityPosture};
use crate::ui::widgets::SeverityBadge;
use chrono::Local;
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use tracing::debug;

/// Security posture page.
pub struct SecurityPage;

impl SecurityPage {
    /// Creates the security posture page.
    #[must_use]
    pub fn new() -> gtk::Box {
        debug!("Creating security page");

        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let header = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header.set_margin_top(16);
        header.set_margin_bottom(12);
        header.set_margin_start(24);
        header.set_margin_end(24);

        let title_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        title_box.set_hexpand(true);

        let title = gtk::Label::new(Some(&tr("Security Posture")));
        title.add_css_class("title-1");
        title.set_halign(gtk::Align::Start);
        title_box.append(&title);

        let subtitle = gtk::Label::new(Some(&tr(
            "Review firewall state, policy enforcement, exposed listeners, privileged accounts, and Flatpak sandboxing.",
        )));
        subtitle.add_css_class("dim-label");
        subtitle.set_wrap(true);
        subtitle.set_xalign(0.0);
        subtitle.set_halign(gtk::Align::Start);
        title_box.append(&subtitle);

        header.append(&title_box);

        let refresh_button = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_button.set_tooltip_text(Some(&tr("Refresh security posture")));
        refresh_button.add_css_class("flat");
        header.append(&refresh_button);

        page.append(&header);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.set_vexpand(true);
        page.append(&content);

        let content_ref = Rc::new(RefCell::new(content));
        let loading = Rc::new(Cell::new(false));

        Self::render_loading(&content_ref.borrow());

        let load: Rc<dyn Fn()> = Rc::new({
            let content = content_ref.clone();
            let loading = loading.clone();
            let refresh_button = refresh_button.clone();

            move || {
                if loading.get() {
                    return;
                }

                loading.set(true);
                refresh_button.set_sensitive(false);
                Self::render_loading(&content.borrow());

                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let posture = SecurityAdapter::collect_posture(false);
                    let _ = tx.send(posture);
                });

                let content = content.clone();
                let loading = loading.clone();
                let refresh_button = refresh_button.clone();
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(posture) => {
                        Self::render_posture(&content.borrow(), &posture);
                        refresh_button.set_sensitive(true);
                        loading.set(false);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        Self::render_error(
                            &content.borrow(),
                            &tr("Failed to load security posture."),
                        );
                        refresh_button.set_sensitive(true);
                        loading.set(false);
                        glib::ControlFlow::Break
                    }
                });
            }
        });

        {
            let load = load.clone();
            refresh_button.connect_clicked(move |_| load());
        }

        glib::idle_add_local_once(move || load());

        page
    }

    fn render_loading(container: &gtk::Box) {
        clear_box(container);

        let loading_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
        loading_box.set_vexpand(true);
        loading_box.set_valign(gtk::Align::Center);
        loading_box.set_halign(gtk::Align::Center);

        let spinner = gtk::Spinner::new();
        spinner.start();
        loading_box.append(&spinner);

        let label = gtk::Label::new(Some(&tr("Collecting security posture...")));
        label.add_css_class("dim-label");
        loading_box.append(&label);

        container.append(&loading_box);
    }

    fn render_error(container: &gtk::Box, message: &str) {
        clear_box(container);

        let status = adw::StatusPage::new();
        status.set_icon_name(Some("dialog-error-symbolic"));
        status.set_title(message);
        container.append(&status);
    }

    fn render_posture(container: &gtk::Box, posture: &SecurityPosture) {
        clear_box(container);

        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(8);
        content.set_margin_bottom(24);
        content.set_margin_start(24);
        content.set_margin_end(24);

        content.append(&Self::build_summary_card(posture));
        content.append(&Self::build_findings_group(posture));
        content.append(&Self::build_exposure_group(posture));
        content.append(&Self::build_policy_group(posture));
        content.append(&Self::build_access_group(posture));
        content.append(&Self::build_flatpak_group(posture));

        scrolled.set_child(Some(&content));
        container.append(&scrolled);
    }

    fn build_summary_card(posture: &SecurityPosture) -> gtk::Box {
        let card = gtk::Box::new(gtk::Orientation::Vertical, 12);
        card.add_css_class("card");

        let title = gtk::Label::new(Some(posture.headline()));
        title.add_css_class("title-2");
        title.set_halign(gtk::Align::Start);
        card.append(&title);

        let timestamp = posture
            .collected_at
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let subtitle = gtk::Label::new(Some(&format!(
            "{} {}",
            tr("Last refreshed:"),
            timestamp
        )));
        subtitle.add_css_class("dim-label");
        subtitle.set_halign(gtk::Align::Start);
        card.append(&subtitle);

        let stats = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        stats.append(&Self::create_stat_box(
            &posture.findings.len().to_string(),
            &tr("Findings"),
        ));
        stats.append(&Self::create_stat_box(
            &posture.public_listener_count().to_string(),
            &tr("Public Listeners"),
        ));
        stats.append(&Self::create_stat_box(
            &posture.risky_flatpak_count().to_string(),
            &tr("Risky Flatpaks"),
        ));
        stats.append(&Self::create_stat_box(
            &posture.recent_denials.to_string(),
            &tr("Recent Denials"),
        ));
        card.append(&stats);

        card
    }

    fn create_stat_box(value: &str, label: &str) -> gtk::Box {
        let stat = gtk::Box::new(gtk::Orientation::Vertical, 4);
        stat.set_hexpand(true);

        let value_label = gtk::Label::new(Some(value));
        value_label.add_css_class("title-1");
        value_label.set_halign(gtk::Align::Start);
        stat.append(&value_label);

        let label_widget = gtk::Label::new(Some(label));
        label_widget.add_css_class("dim-label");
        label_widget.add_css_class("caption");
        label_widget.set_halign(gtk::Align::Start);
        stat.append(&label_widget);

        stat
    }

    fn build_findings_group(posture: &SecurityPosture) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        group.set_title(&tr("Findings"));
        group.set_description(Some(&tr("Deterministic checks derived from the collected local state")));

        for finding in &posture.findings {
            let row = adw::ExpanderRow::new();
            row.set_title(&finding.title);
            row.set_subtitle(&finding.summary);

            let badge = finding_badge(finding);
            row.add_suffix(&badge);

            let details = gtk::Box::new(gtk::Orientation::Vertical, 6);
            details.set_margin_top(8);
            details.set_margin_bottom(8);
            details.set_margin_start(8);
            details.set_margin_end(8);

            if finding.evidence.is_empty() {
                let empty = gtk::Label::new(Some(&tr("No additional evidence")));
                empty.add_css_class("dim-label");
                empty.set_halign(gtk::Align::Start);
                details.append(&empty);
            } else {
                for evidence in &finding.evidence {
                    let label = gtk::Label::new(Some(evidence));
                    label.set_halign(gtk::Align::Start);
                    label.set_xalign(0.0);
                    label.set_wrap(true);
                    label.add_css_class("caption");
                    details.append(&label);
                }
            }

            row.add_row(&details);
            group.add(&row);
        }

        group
    }

    fn build_exposure_group(posture: &SecurityPosture) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        group.set_title(&tr("Exposure"));
        group.set_description(Some(&tr("Network listeners, firewall status, and SSH reachability")));

        let firewall_summary = posture
            .state
            .firewall
            .summary
            .as_deref()
            .unwrap_or_else(|| posture.state.firewall.backend.label());
        group.add(&Self::create_value_row(
            &tr("Firewall"),
            firewall_summary,
            if posture.state.firewall.active {
                tr("Active")
            } else {
                tr("Inactive")
            },
            if posture.state.firewall.active {
                "success"
            } else {
                "error"
            },
        ));

        group.add(&Self::create_value_row(
            &tr("Public listeners"),
            &tr("Sockets bound beyond loopback"),
            &posture.public_listener_count().to_string(),
            if posture.public_listener_count() == 0 {
                "success"
            } else {
                "warning"
            },
        ));

        for socket in posture
            .state
            .listening_sockets
            .iter()
            .filter(|socket| socket.public)
            .take(10)
        {
            let row = adw::ActionRow::new();
            row.set_title(&format!(
                "{} {}:{}",
                socket.protocol, socket.bind_address, socket.port
            ));
            if let Some(process) = &socket.process {
                row.set_subtitle(process);
            } else {
                row.set_subtitle(&tr("Process not visible"));
            }
            group.add(&row);
        }

        group.add(&Self::create_value_row(
            &tr("SSH"),
            &format!(
                "{}: {}",
                tr("Listen addresses"),
                if posture.state.ssh.listening_addresses.is_empty() {
                    tr("none")
                } else {
                    posture.state.ssh.listening_addresses.join(", ")
                }
            ),
            if posture.state.ssh.service_active {
                tr("Active")
            } else {
                tr("Inactive")
            },
            if posture.state.ssh.service_active {
                "warning"
            } else {
                "success"
            },
        ));

        group
    }

    fn build_policy_group(posture: &SecurityPosture) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        group.set_title(&tr("Policy Enforcement"));
        group.set_description(Some(&tr("Mandatory access control and Secure Boot status")));

        group.add(&Self::create_value_row(
            &tr("SELinux"),
            &tr("Current enforcement mode"),
            posture.state.mac_policy.selinux.label(),
            policy_css_class(posture.state.mac_policy.selinux, false),
        ));
        group.add(&Self::create_value_row(
            &tr("AppArmor"),
            &format!(
                "{} {}, {} {}",
                posture.state.mac_policy.apparmor_enforce_profiles,
                tr("enforce"),
                posture.state.mac_policy.apparmor_complain_profiles,
                tr("complain")
            ),
            posture.state.mac_policy.apparmor.label(),
            policy_css_class(posture.state.mac_policy.apparmor, true),
        ));
        group.add(&Self::create_value_row(
            &tr("Secure Boot"),
            &tr("UEFI Secure Boot state"),
            posture.state.secure_boot.label(),
            if posture.state.secure_boot == crate::domain::snapshot::SecureBootState::Enabled {
                "success"
            } else if posture.state.secure_boot
                == crate::domain::snapshot::SecureBootState::Disabled
            {
                "warning"
            } else {
                "dim-label"
            },
        ));
        group.add(&Self::create_value_row(
            &tr("Recent denials"),
            &tr("SELinux/AppArmor denial events from the last 24 hours"),
            &posture.recent_denials.to_string(),
            if posture.recent_denials == 0 {
                "success"
            } else {
                "warning"
            },
        ));

        group
    }

    fn build_access_group(posture: &SecurityPosture) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        group.set_title(&tr("Privileged Access"));
        group.set_description(Some(&tr("Users discovered in local sudo or wheel-style groups")));

        group.add(&Self::create_value_row(
            &tr("Admin-capable accounts"),
            &tr("Members of sudo, wheel, or admin groups"),
            &posture.state.admin_accounts.len().to_string(),
            if posture.state.admin_accounts.len() <= 1 {
                "success"
            } else {
                "warning"
            },
        ));

        for account in &posture.state.admin_accounts {
            let row = adw::ActionRow::new();
            row.set_title(&account.username);
            row.set_subtitle(&account.groups.join(", "));
            group.add(&row);
        }

        group
    }

    fn build_flatpak_group(posture: &SecurityPosture) -> adw::PreferencesGroup {
        let group = adw::PreferencesGroup::new();
        group.set_title(&tr("Flatpak Sandboxing"));
        group.set_description(Some(&tr("Applications with broad filesystem, network, device, or bus access")));

        group.add(&Self::create_value_row(
            &tr("Apps with broad grants"),
            &tr("Flatpak apps with weaker sandbox boundaries"),
            &posture.risky_flatpak_count().to_string(),
            if posture.risky_flatpak_count() == 0 {
                "success"
            } else {
                "warning"
            },
        ));

        for (app_id, permissions) in posture.state.flatpak_permissions.iter().take(12) {
            let row = adw::ActionRow::new();
            row.set_title(app_id);
            row.set_subtitle(&permissions.broad_permissions.join(", "));
            group.add(&row);
        }

        group
    }

    fn create_value_row(
        title: &str,
        subtitle: impl AsRef<str>,
        value: impl AsRef<str>,
        css_class: &str,
    ) -> adw::ActionRow {
        let row = adw::ActionRow::builder().title(title).build();
        row.set_subtitle(subtitle.as_ref());

        let label = gtk::Label::new(Some(value.as_ref()));
        label.add_css_class(css_class);
        row.add_suffix(&label);

        row
    }
}

fn clear_box(container: &gtk::Box) {
    while let Some(child) = container.first_child() {
        container.remove(&child);
    }
}

fn finding_badge(finding: &SecurityFinding) -> gtk::Label {
    let severity = match finding.severity {
        Severity::Critical => Severity::Critical,
        Severity::Error => Severity::Error,
        Severity::Warning => Severity::Warning,
        Severity::Info => Severity::Info,
    };
    SeverityBadge::new(severity)
}

fn policy_css_class(mode: crate::domain::snapshot::PolicyMode, apparmor: bool) -> &'static str {
    match mode {
        crate::domain::snapshot::PolicyMode::Enforcing => "success",
        crate::domain::snapshot::PolicyMode::Permissive => "warning",
        crate::domain::snapshot::PolicyMode::Complain => {
            if apparmor {
                "warning"
            } else {
                "dim-label"
            }
        }
        crate::domain::snapshot::PolicyMode::Disabled => "error",
        crate::domain::snapshot::PolicyMode::NotInstalled
        | crate::domain::snapshot::PolicyMode::Unknown => "dim-label",
    }
}
