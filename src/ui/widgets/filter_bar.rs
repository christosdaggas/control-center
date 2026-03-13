//! Filter bar widget.

use crate::application::state::SharedState;
use crate::domain::filter::FilterPreset;
use crate::i18n::tr;
use gtk4::prelude::*;
use gtk4::{SearchEntry, ToggleButton};
use tracing::debug;

/// Filter bar with presets and search.
pub struct FilterBar;

impl FilterBar {
    /// Creates a new filter bar.
    #[must_use]
    pub fn new(state: &SharedState) -> gtk4::Box {
        let container = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        container.add_css_class("filter-bar");
        container.set_margin_start(16);
        container.set_margin_end(16);
        container.set_margin_top(8);
        container.set_margin_bottom(8);


        let presets_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        presets_box.add_css_class("linked");

        let since_reboot_btn = Self::create_preset_button(&tr("Since Reboot"), true);
        let errors_btn = Self::create_preset_button(&tr("Errors"), false);
        let warnings_btn = Self::create_preset_button(&tr("Warnings"), false);
        let changes_btn = Self::create_preset_button(&tr("Changes"), false);
        let all_btn = Self::create_preset_button(&tr("All"), false);


        let buttons = vec![
            (since_reboot_btn.clone(), FilterPreset::SinceLastReboot),
            (errors_btn.clone(), FilterPreset::ErrorsOnly),
            (warnings_btn.clone(), FilterPreset::WarningsOnly),
            (changes_btn.clone(), FilterPreset::ChangesOnly),
            (all_btn.clone(), FilterPreset::All),
        ];

        for (btn, preset) in &buttons {
            let buttons_clone: Vec<_> = buttons.iter().map(|(b, _)| b.clone()).collect();
            let state_clone = state.clone();
            let preset_clone = *preset;

            btn.connect_toggled(move |button| {
                if button.is_active() {

                    for other in &buttons_clone {
                        if other != button {
                            other.set_active(false);
                        }
                    }
                    debug!(preset = ?preset_clone, "Filter preset changed");
                    

                    if let Ok(mut s) = state_clone.write() {

                        s.filter_config.min_severity = None;
                        s.filter_config.max_severity = None;
                        s.filter_config.include_types.clear();
                        s.filter_config.time_start = None;
                        s.filter_config.time_end = None;

                        s.filter_config.preset = preset_clone;
                        

                        match preset_clone {
                            FilterPreset::All => {

                            }
                            FilterPreset::ErrorsOnly => {
                                s.filter_config.min_severity = Some(crate::domain::event::Severity::Error);
                            }
                            FilterPreset::WarningsOnly => {
                                // Only warnings - exact match (min=Warning, max=Warning)
                                s.filter_config.min_severity = Some(crate::domain::event::Severity::Warning);
                                s.filter_config.max_severity = Some(crate::domain::event::Severity::Warning);
                            }
                            FilterPreset::ChangesOnly => {
                                s.filter_config.include_types = vec![
                                    crate::domain::event::EventType::PackageInstall,
                                    crate::domain::event::EventType::PackageUpdate,
                                    crate::domain::event::EventType::PackageRemove,
                                    crate::domain::event::EventType::ServiceStart,
                                    crate::domain::event::EventType::ServiceStop,
                                    crate::domain::event::EventType::ServiceRestart,
                                    crate::domain::event::EventType::ServiceFailed,
                                    crate::domain::event::EventType::SystemBoot,
                                ];
                            }
                            FilterPreset::SinceLastReboot => {
                                // For since reboot, we'd need boot time - for now just use last hour
                                s.filter_config.time_start = Some(chrono::Utc::now() - chrono::Duration::hours(1));
                            }
                            _ => {}
                        }
                        

                        s.apply_filter();
                        debug!(filtered_count = s.filtered_events.len(), "Filter applied");
                    }
                    

                    button.activate_action("win.refresh-ui", None).ok();
                }
            });
        }

        presets_box.append(&since_reboot_btn);
        presets_box.append(&errors_btn);
        presets_box.append(&warnings_btn);
        presets_box.append(&changes_btn);
        presets_box.append(&all_btn);

        container.append(&presets_box);


        let spacer = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        container.append(&spacer);


        let search = SearchEntry::builder()
            .placeholder_text(&tr("Search events..."))
            .width_chars(25)
            .build();

        let state_for_search = state.clone();
        let search_clone = search.clone();
        search.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            debug!(query = %query, "Search changed");
            

            if let Ok(mut s) = state_for_search.write() {
                if query.is_empty() {
                    s.filter_config.search_query = None;
                } else {
                    s.filter_config.search_query = Some(query);
                }
                

                s.apply_filter();
                debug!(filtered_count = s.filtered_events.len(), "Search filter applied");
            }
            

            search_clone.activate_action("win.refresh-ui", None).ok();
        });

        container.append(&search);

        container
    }

    fn create_preset_button(label: &str, active: bool) -> ToggleButton {
        let button = ToggleButton::builder()
            .label(label)
            .active(active)
            .build();

        button
    }
}
