//! Help and documentation page.
//!
//! Provides user guidance and application information.

use crate::i18n::tr;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation, PolicyType, ScrolledWindow};





/// Creates the help page widget.
pub fn create_help_page() -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);

    // Page header
    let header_box = GtkBox::new(Orientation::Vertical, 4);
    header_box.set_margin_start(24);
    header_box.set_margin_end(24);
    header_box.set_margin_top(24);
    header_box.set_margin_bottom(12);

    let title = Label::new(Some(&tr("Help")));
    title.add_css_class("title-1");
    title.set_halign(gtk4::Align::Start);
    header_box.append(&title);

    let subtitle = Label::new(Some(&tr("Learn how to use Control Center")));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk4::Align::Start);
    header_box.append(&subtitle);

    page.append(&header_box);

    // Scrollable content
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_policy(PolicyType::Never, PolicyType::Automatic);

    let content_box = GtkBox::new(Orientation::Vertical, 24);
    content_box.set_margin_start(24);
    content_box.set_margin_end(24);
    content_box.set_margin_top(12);
    content_box.set_margin_bottom(24);



    // About section
    content_box.append(&create_section(
        &tr("About Control Center"),
        &format!(
            "{}
{}: MIT",
            tr("Control Center is a modern system monitoring and management tool for Linux. \
             It provides a graphical interface for monitoring system health, managing services, \
             viewing activity logs, and comparing system state snapshots."),
            tr("License")
        ),
    ));

    // System Health section
    content_box.append(&create_section(
        &tr("System Health"),
        &tr("The System Health page displays your system's current status including CPU usage, \
         memory utilization, disk space, and uptime. Use this page to get a quick overview \
         of your system's performance and resource consumption."),
    ));

    // Activity Timeline section
    content_box.append(&create_section(
        &tr("Activity Timeline"),
        &tr("The Activity Timeline shows system events, service changes, and log entries \
         in chronological order. Filter events by severity level and source to focus \
         on what matters. This helps you track what's happening on your system."),
    ));

    // Security Posture section
    content_box.append(&create_section(
        &tr("Security Posture"),
        &tr("The Security Posture page reviews firewall state, SELinux/AppArmor mode, \
         Secure Boot, exposed listening ports, SSH exposure, privileged local accounts, \
         and Flatpak sandbox permissions. Use it to spot risky exposure and compare \
         posture drift against your snapshots."),
    ));

    // Services Manager section
    content_box.append(&create_section(
        &tr("Services Manager"),
        &tr("The Services Manager lets you view and control systemd services. \
         Start, stop, enable, or disable services with a simple interface. \
         Monitor service status and quickly identify failed services that may \
         indicate system problems."),
    ));

    // Known-Good Snapshot section
    content_box.append(&create_section(
        &tr("Known-Good Snapshot"),
        &tr("The Known-Good Snapshot feature captures your system's state at a point in time. \
         Compare snapshots to identify what changed—installed packages, service configurations, \
         Flatpak apps, network settings, and more. Create a baseline after a fresh install \
         to track future changes."),
    ));

    // Tips section
    content_box.append(&create_section(
        &tr("Tips"),
        &tr("• Create a baseline snapshot after installing your system or major updates.\n\
         • Pay attention to 'High Impact' changes—they often affect security or stability.\n\
         • Use 'Redact sensitive data' when sharing snapshots for troubleshooting.\n\
         • Regularly check for failed services that might indicate problems.\n\
         • Use keyboard shortcuts for faster navigation (Alt+1...5 to switch pages)."),
    ));

    scroll.set_child(Some(&content_box));
    page.append(&scroll);

    page
}

/// Creates a section with title and description.
fn create_section(title: &str, description: &str) -> GtkBox {
    let section = GtkBox::new(Orientation::Vertical, 8);

    let title_label = Label::new(Some(title));
    title_label.add_css_class("title-3");
    title_label.set_halign(gtk4::Align::Start);
    section.append(&title_label);

    let desc_label = Label::new(Some(description));
    desc_label.set_wrap(true);
    desc_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    desc_label.set_xalign(0.0);
    desc_label.set_halign(gtk4::Align::Start);
    section.append(&desc_label);

    section
}
