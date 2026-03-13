//! Services Manager Page - Unified services and startup orchestrator.
//!
//! Lists system and user services with start/stop/enable/disable controls.

use crate::i18n::tr;
use crate::infrastructure::adapters::{
    EnabledState, SystemdAdapter, SystemdUnit, UnitState, UnitType,
};
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::{debug, info, warn};

/// Filter options for services list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceFilter {
    /// Show all services.
    #[default]
    All,
    /// Show only running services.
    Running,
    /// Show only failed services.
    Failed,
    /// Show only enabled services.
    Enabled,
    /// Show only disabled services.
    Disabled,
}

impl ServiceFilter {
    /// Returns all filter options.
    pub fn all() -> &'static [Self] {
        &[
            Self::All,
            Self::Running,
            Self::Failed,
            Self::Enabled,
            Self::Disabled,
        ]
    }

    /// Display name for the filter.
    pub fn display_name(&self) -> String {
        match self {
            Self::All => tr("All"),
            Self::Running => tr("Running"),
            Self::Failed => tr("Failed"),
            Self::Enabled => tr("Enabled"),
            Self::Disabled => tr("Disabled"),
        }
    }

    /// Checks if a unit matches this filter.
    pub fn matches(&self, unit: &SystemdUnit) -> bool {
        match self {
            Self::All => true,
            Self::Running => unit.state == UnitState::Active,
            Self::Failed => unit.state == UnitState::Failed,
            Self::Enabled => unit.enabled == EnabledState::Enabled,
            Self::Disabled => unit.enabled == EnabledState::Disabled,
        }
    }
}

/// View mode for services.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceViewMode {
    /// System-level services.
    #[default]
    System,
    /// User-level services.
    User,
    /// Timer units.
    Timers,
}

impl ServiceViewMode {
    /// Returns all view modes.
    pub fn all() -> &'static [Self] {
        &[Self::System, Self::User, Self::Timers]
    }

    /// Display name.
    pub fn display_name(&self) -> String {
        match self {
            Self::System => tr("System Services"),
            Self::User => tr("User Services"),
            Self::Timers => tr("Timers"),
        }
    }

    /// Icon name.
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::System => "computer-symbolic",
            Self::User => "avatar-default-symbolic",
            Self::Timers => "alarm-symbolic",
        }
    }
}

/// Services manager page.
pub struct ServicesPage;

impl ServicesPage {
    /// Creates the services manager page.
    pub fn new() -> gtk::Box {
        debug!("Creating services page");

        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        page.add_css_class("services-page");


        let units: Rc<RefCell<Vec<SystemdUnit>>> = Rc::new(RefCell::new(Vec::new()));
        let current_filter: Rc<RefCell<ServiceFilter>> = Rc::new(RefCell::new(ServiceFilter::All));
        let current_mode: Rc<RefCell<ServiceViewMode>> =
            Rc::new(RefCell::new(ServiceViewMode::System));
        let search_text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));


        let header_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        header_box.set_margin_top(12);
        header_box.set_margin_bottom(12);
        header_box.set_margin_start(16);
        header_box.set_margin_end(16);


        let mode_model = gtk::StringList::new(&[]);
        for mode in ServiceViewMode::all() {
            mode_model.append(&mode.display_name());
        }
        let mode_dropdown = gtk::DropDown::new(Some(mode_model), gtk::Expression::NONE);
        mode_dropdown.set_selected(0);
        header_box.append(&mode_dropdown);


        let filter_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        filter_box.add_css_class("linked");

        for filter in ServiceFilter::all() {
            let btn = gtk::ToggleButton::with_label(&filter.display_name());
            btn.set_active(*filter == ServiceFilter::All);
            filter_box.append(&btn);
        }
        header_box.append(&filter_box);


        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        header_box.append(&spacer);


        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some(&tr("Search services...")));
        search_entry.set_width_chars(30);
        header_box.append(&search_entry);


        let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.set_tooltip_text(Some(&tr("Refresh services")));
        refresh_btn.add_css_class("flat");
        header_box.append(&refresh_btn);

        page.append(&header_box);


        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        page.append(&separator);


        let scrolled = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let list_box = gtk::ListBox::new();
        list_box.set_selection_mode(gtk::SelectionMode::None);
        list_box.add_css_class("boxed-list");
        list_box.set_margin_top(12);
        list_box.set_margin_bottom(12);
        list_box.set_margin_start(16);
        list_box.set_margin_end(16);

        scrolled.set_child(Some(&list_box));
        page.append(&scrolled);


        let status_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        status_bar.set_margin_start(16);
        status_bar.set_margin_end(16);
        status_bar.set_margin_top(8);
        status_bar.set_margin_bottom(8);

        let status_label = gtk::Label::new(Some(&tr("Loading services...")));
        status_label.add_css_class("dim-label");
        status_label.set_halign(gtk::Align::Start);
        status_bar.append(&status_label);

        page.append(&status_bar);


        let list_box_ref = list_box.clone();
        let status_label_ref = status_label.clone();
        let units_ref = units.clone();
        let filter_ref = current_filter.clone();
        let mode_ref = current_mode.clone();
        let search_ref = search_text.clone();


        let update_list = {
            let list_box = list_box_ref.clone();
            let status_label = status_label_ref.clone();
            let units = units_ref.clone();
            let filter = filter_ref.clone();
            let search = search_ref.clone();

            move || {
                let units = units.borrow();
                let filter = *filter.borrow();
                let search_text = search.borrow().to_lowercase();


                while let Some(child) = list_box.first_child() {
                    list_box.remove(&child);
                }


                let filtered: Vec<_> = units
                    .iter()
                    .filter(|u| {
                        filter.matches(u)
                            && (search_text.is_empty()
                                || u.name.to_lowercase().contains(&search_text)
                                || u.description.to_lowercase().contains(&search_text))
                    })
                    .collect();

                if filtered.is_empty() {
                    let empty_row = adw::ActionRow::new();
                    empty_row.set_title(&tr("No services match the current filter"));
                    let info_icon = gtk::Image::from_icon_name("dialog-information-symbolic");
                    empty_row.add_prefix(&info_icon);
                    list_box.append(&empty_row);
                } else {
                    for unit in filtered.iter() {
                        let row = Self::create_service_row(unit);
                        list_box.append(&row);
                    }
                }

                status_label.set_label(&format!(
                    "{} {} {} {} {}",
                    tr("Showing"),
                    filtered.len(),
                    tr("of"),
                    units.len(),
                    tr("services")
                ));
            }
        };


        let load_services = {
            let units = units_ref.clone();
            let mode = mode_ref.clone();
            let update_list = update_list.clone();
            let status_label = status_label_ref.clone();

            move || {
                let mode = *mode.borrow();
                let (unit_type, user) = match mode {
                    ServiceViewMode::System => (Some(UnitType::Service), false),
                    ServiceViewMode::User => (Some(UnitType::Service), true),
                    ServiceViewMode::Timers => (Some(UnitType::Timer), false),
                };

                status_label.set_label(&tr("Loading services..."));
                debug!(mode = ?mode, "Queueing service load");

                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let loaded = SystemdAdapter::list_units(unit_type, user);
                    let _ = tx.send(loaded);
                });

                // Poll the channel from the main thread
                let units = units.clone();
                let update_list = update_list.clone();
                glib::idle_add_local(move || {
                    match rx.try_recv() {
                        Ok(loaded) => {
                            info!(count = loaded.len(), "Services loaded (async)");
                            *units.borrow_mut() = loaded;
                            update_list();
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
                    }
                });
            }
        };


        let load_clone = load_services.clone();
        glib::idle_add_local_once(move || {
            load_clone();
        });


        {
            let mode = mode_ref.clone();
            let load = load_services.clone();
            mode_dropdown.connect_selected_notify(move |dropdown| {
                let idx = dropdown.selected() as usize;
                if let Some(new_mode) = ServiceViewMode::all().get(idx) {
                    *mode.borrow_mut() = *new_mode;
                    load();
                }
            });
        }


        {
            let mut filter_btns: Vec<gtk::ToggleButton> = Vec::new();
            let mut child = filter_box.first_child();
            while let Some(widget) = child {
                if let Ok(btn) = widget.clone().downcast::<gtk::ToggleButton>() {
                    filter_btns.push(btn);
                }
                child = widget.next_sibling();
            }

            for (idx, btn) in filter_btns.iter().enumerate() {
                let btns = filter_btns.clone();
                let filter = filter_ref.clone();
                let update = update_list.clone();

                btn.connect_toggled(move |clicked_btn| {
                    if clicked_btn.is_active() {

                        for (i, b) in btns.iter().enumerate() {
                            if i != idx {
                                b.set_active(false);
                            }
                        }

                        if let Some(new_filter) = ServiceFilter::all().get(idx) {
                            *filter.borrow_mut() = *new_filter;
                            update();
                        }
                    }
                });
            }
        }


        {
            let search = search_ref.clone();
            let update = update_list.clone();
            search_entry.connect_search_changed(move |entry| {
                *search.borrow_mut() = entry.text().to_string();
                update();
            });
        }


        {
            let load = load_services.clone();
            refresh_btn.connect_clicked(move |_| {
                load();
            });
        }

        page
    }

    /// Creates a row for a service unit.
    fn create_service_row(unit: &SystemdUnit) -> adw::ExpanderRow {
        let row = adw::ExpanderRow::new();
        row.set_title(&unit.short_name());
        row.set_subtitle(&unit.description);
        let unit_icon = gtk::Image::from_icon_name(unit.unit_type.icon_name());
        row.add_prefix(&unit_icon);


        let state_label = gtk::Label::new(Some(unit.state.display_name()));
        state_label.add_css_class(unit.state.css_class());
        state_label.set_valign(gtk::Align::Center);
        row.add_suffix(&state_label);


        let enabled_label = gtk::Label::new(Some(unit.enabled.display_name()));
        enabled_label.add_css_class("dim-label");
        enabled_label.set_valign(gtk::Align::Center);
        enabled_label.set_margin_start(8);
        row.add_suffix(&enabled_label);


        if unit.is_critical() {
            let critical_icon = gtk::Image::from_icon_name("dialog-warning-symbolic");
            critical_icon.set_tooltip_text(Some(&tr("Critical system service")));
            critical_icon.add_css_class("warning");
            critical_icon.set_margin_start(8);
            row.add_suffix(&critical_icon);
        }


        let details_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        details_box.set_margin_top(8);
        details_box.set_margin_bottom(8);
        details_box.set_margin_start(8);
        details_box.set_margin_end(8);


        let actions_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);

        let unit_name = unit.name.clone();
        let is_user = unit.is_user;


        let start_btn = gtk::Button::with_label(&tr("Start"));
        start_btn.add_css_class("suggested-action");
        start_btn.set_sensitive(unit.state != UnitState::Active);
        {
            let name = unit_name.clone();
            start_btn.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                match SystemdAdapter::start(&name, is_user) {
                    Ok(()) => info!(unit = %name, "Service started"),
                    Err(e) => warn!(error = %e, unit = %name, "Failed to start service"),
                }
            });
        }
        actions_box.append(&start_btn);


        let stop_btn = gtk::Button::with_label(&tr("Stop"));
        stop_btn.add_css_class("destructive-action");
        stop_btn.set_sensitive(unit.state == UnitState::Active);
        if unit.is_critical() {
            stop_btn.set_tooltip_text(Some(&tr("Warning: This is a critical system service")));
        }
        {
            let name = unit_name.clone();
            stop_btn.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                match SystemdAdapter::stop(&name, is_user) {
                    Ok(()) => info!(unit = %name, "Service stopped"),
                    Err(e) => warn!(error = %e, unit = %name, "Failed to stop service"),
                }
            });
        }
        actions_box.append(&stop_btn);


        let restart_btn = gtk::Button::with_label(&tr("Restart"));
        {
            let name = unit_name.clone();
            restart_btn.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                match SystemdAdapter::restart(&name, is_user) {
                    Ok(()) => info!(unit = %name, "Service restarted"),
                    Err(e) => warn!(error = %e, unit = %name, "Failed to restart service"),
                }
            });
        }
        actions_box.append(&restart_btn);


        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        actions_box.append(&spacer);


        if unit.enabled.can_toggle() {
            let enable_switch = gtk::Switch::new();
            enable_switch.set_active(unit.enabled == EnabledState::Enabled);
            enable_switch.set_valign(gtk::Align::Center);

            let enable_label = gtk::Label::new(Some(&tr("Enable at boot")));
            enable_label.add_css_class("dim-label");

            let enable_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            enable_box.append(&enable_label);
            enable_box.append(&enable_switch);

            {
                let name = unit_name.clone();
                enable_switch.connect_state_set(move |_, state| {
                    let result = if state {
                        SystemdAdapter::enable(&name, is_user)
                    } else {
                        SystemdAdapter::disable(&name, is_user)
                    };

                    match result {
                        Ok(()) => info!(unit = %name, enabled = state, "Service enable state changed"),
                        Err(e) => warn!(error = %e, unit = %name, "Failed to change enable state"),
                    }

                    glib::Propagation::Proceed
                });
            }

            actions_box.append(&enable_box);
        }

        details_box.append(&actions_box);


        let logs_expander = gtk::Expander::new(Some(&tr("Recent Logs")));
        logs_expander.set_margin_top(8);

        let logs_view = gtk::TextView::new();
        logs_view.set_editable(false);
        logs_view.set_cursor_visible(false);
        logs_view.set_monospace(true);
        logs_view.set_wrap_mode(gtk::WrapMode::Word);
        logs_view.add_css_class("card");

        let logs_scroll = gtk::ScrolledWindow::new();
        logs_scroll.set_min_content_height(150);
        logs_scroll.set_child(Some(&logs_view));


        {
            let name = unit_name.clone();
            let view = logs_view.clone();
            logs_expander.connect_expanded_notify(move |expander| {
                if expander.is_expanded() {
                    let logs = SystemdAdapter::get_unit_logs(&name, is_user, 20);
                    let buffer = view.buffer();
                    buffer.set_text(&logs.join("\n"));
                }
            });
        }

        logs_expander.set_child(Some(&logs_scroll));
        details_box.append(&logs_expander);

        row.add_row(&details_box);

        row
    }
}
