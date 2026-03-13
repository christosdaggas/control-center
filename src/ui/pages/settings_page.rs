//! Settings / Preferences page.
//!
//! Uses `adw::PreferencesPage` to expose configuration knobs to the user
//! and persists changes to the XDG config file.

use crate::config::{Config, DefaultFilterPreset, ThemePreference};
use crate::i18n::tr;

use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::{debug, warn};

/// Creates the Settings preferences page.
#[must_use]
pub fn create_settings_page() -> gtk4::Box {
    let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(24);
    page.set_margin_end(24);
    
    // Add title manually since we aren't using PreferencesPage
    let title = gtk4::Label::new(Some(&tr("Settings")));
    title.add_css_class("title-1");
    title.set_halign(gtk4::Align::Start);
    title.set_margin_bottom(24);
    page.append(&title);

    let config = Config::load().unwrap_or_default();

    // ── Appearance group ──────────────────────────────────────────
    let appearance_group = adw::PreferencesGroup::new();
    appearance_group.set_title(&tr("Appearance"));
    appearance_group.set_description(Some(&tr("Customize the look and feel")));


    let theme_row = adw::ComboRow::new();
    theme_row.set_title(&tr("Theme"));
    theme_row.set_subtitle(&tr("Choose light, dark, or follow the system theme"));
    let theme_model = gtk4::StringList::new(&[&tr("System"), &tr("Light"), &tr("Dark")]);
    theme_row.set_model(Some(&theme_model));
    theme_row.set_selected(match config.theme {
        ThemePreference::System => 0,
        ThemePreference::Light => 1,
        ThemePreference::Dark => 2,
    });
    theme_row.connect_selected_notify(|row| {
        let pref = match row.selected() {
            1 => ThemePreference::Light,
            2 => ThemePreference::Dark,
            _ => ThemePreference::System,
        };
        update_config(|c| c.theme = pref);


        let style = adw::StyleManager::default();
        match pref {
            ThemePreference::System => style.set_color_scheme(adw::ColorScheme::Default),
            ThemePreference::Light => style.set_color_scheme(adw::ColorScheme::ForceLight),
            ThemePreference::Dark => style.set_color_scheme(adw::ColorScheme::ForceDark),
        }
    });
    appearance_group.add(&theme_row);
    page.append(&appearance_group);

    // ── Timeline defaults group ───────────────────────────────────
    let timeline_group = adw::PreferencesGroup::new();
    timeline_group.set_margin_top(24);
    timeline_group.set_title(&tr("Timeline"));
    timeline_group.set_description(Some(&tr("Configure default timeline behaviour")));


    let filter_row = adw::ComboRow::new();
    filter_row.set_title(&tr("Default filter"));
    filter_row.set_subtitle(&tr("Filter preset applied when the app starts"));
    let filter_model = gtk4::StringList::new(&[
        &tr("All events"),
        &tr("Since last reboot"),
        &tr("Warnings & errors"),
        &tr("Changes only"),
    ]);
    filter_row.set_model(Some(&filter_model));
    filter_row.set_selected(match config.default_filter {
        DefaultFilterPreset::All => 0,
        DefaultFilterPreset::SinceLastReboot => 1,
        DefaultFilterPreset::WarningsAndErrors => 2,
        DefaultFilterPreset::ChangesOnly => 3,
    });
    filter_row.connect_selected_notify(|row| {
        let preset = match row.selected() {
            1 => DefaultFilterPreset::SinceLastReboot,
            2 => DefaultFilterPreset::WarningsAndErrors,
            3 => DefaultFilterPreset::ChangesOnly,
            _ => DefaultFilterPreset::All,
        };
        update_config(|c| c.default_filter = preset);
    });
    timeline_group.add(&filter_row);


    let history_row = adw::SpinRow::new(
        Some(&gtk4::Adjustment::new(
            f64::from(config.default_history_days),
            1.0,
            90.0,
            1.0,
            7.0,
            0.0,
        )),
        1.0,
        0,
    );
    history_row.set_title(&tr("History (days)"));
    history_row.set_subtitle(&tr("Number of days of events to load by default"));
    history_row.connect_value_notify(|row| {
        let days = row.value() as u32;
        update_config(|c| c.default_history_days = days);
    });
    timeline_group.add(&history_row);

    page.append(&timeline_group);

    // ── Data management group ─────────────────────────────────────
    let data_group = adw::PreferencesGroup::new();
    data_group.set_margin_top(24);
    data_group.set_title(&tr("Data"));
    data_group.set_description(Some(&tr("Manage stored data and retention")));


    let retention_row = adw::SpinRow::new(
        Some(&gtk4::Adjustment::new(
            f64::from(config.data_retention_days),
            0.0,
            365.0,
            1.0,
            30.0,
            0.0,
        )),
        1.0,
        0,
    );
    retention_row.set_title(&tr("Snapshot retention (days)"));
    retention_row.set_subtitle(&tr("Automatically delete snapshots older than this (0 = keep forever)"));
    retention_row.connect_value_notify(|row| {
        let days = row.value() as u32;
        update_config(|c| c.data_retention_days = days);
    });
    data_group.add(&retention_row);


    let diag_row = adw::SwitchRow::new();
    diag_row.set_title(&tr("Diagnostic mode"));
    diag_row.set_subtitle(&tr("Enable verbose logging for troubleshooting"));
    diag_row.set_active(config.diagnostic_mode);
    diag_row.connect_active_notify(|row| {
        let active = row.is_active();
        update_config(|c| c.diagnostic_mode = active);
    });
    data_group.add(&diag_row);

    page.append(&data_group);

    // ── Notifications group ───────────────────────────────────────
    let notif_group = adw::PreferencesGroup::new();
    notif_group.set_margin_top(24);
    notif_group.set_title(&tr("Notifications"));
    notif_group.set_description(Some(&tr("Desktop notification settings")));

    let notif_row = adw::SwitchRow::new();
    notif_row.set_title(&tr("Enable notifications"));
    notif_row.set_subtitle(&tr("Show desktop notifications for critical events"));
    notif_row.set_active(config.notifications_enabled);
    notif_row.connect_active_notify(|row| {
        let active = row.is_active();
        update_config(|c| c.notifications_enabled = active);
    });
    notif_group.add(&notif_row);

    page.append(&notif_group);

    page
}

/// Loads the current config, applies a mutation, and saves it.
fn update_config(f: impl FnOnce(&mut Config)) {
    let mut config = Config::load().unwrap_or_default();
    f(&mut config);
    if let Err(e) = config.save() {
        warn!(error = %e, "Failed to save settings");
    } else {
        debug!("Settings saved");
    }
}
