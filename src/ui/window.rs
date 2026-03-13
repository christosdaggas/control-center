//! Main application window with sidebar navigation.

use crate::application::state::{create_shared_state, SharedState};
use crate::application::services::create_services;
use crate::infrastructure::desktop::detect_desktop;
use crate::ui::pages::{SecurityPage, SystemHealthPage, ServicesPage, PerformancePage, create_snapshot_page, create_help_page, create_settings_page};
use crate::ui::widgets::{FilterBar, ThemePopover, TimelineView};
use chrono::Utc;
use gtk4::prelude::*;
use gtk4::gio;
use gtk4::glib;
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::{debug, info, error};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::i18n::tr;

/// Application name constant.
const APP_NAME: &str = "Control Center";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Navigation items for the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    /// System Activity Timeline - shows system events.
    SystemTimeline,
    /// System Health dashboard.
    SystemHealth,
    /// Performance / Resource Pressure Analyzer.
    Performance,
    /// Security posture review.
    Security,
    /// Services Manager.
    ServicesManager,
    /// Known-Good Snapshot comparison.
    Snapshots,
    /// Settings / Preferences.
    Settings,
    /// Help & Documentation.
    Help,
}

impl NavItem {
    /// Returns the icon name for this navigation item.
    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::SystemTimeline => "view-list-symbolic",
            Self::SystemHealth => "computer-symbolic",
            Self::Performance => "applications-system-symbolic",
            Self::Security => "security-high-symbolic",
            Self::ServicesManager => "emblem-system-symbolic",
            Self::Snapshots => "document-save-symbolic",
            Self::Settings => "preferences-system-symbolic",
            Self::Help => "help-about-symbolic",
        }
    }

    /// Returns the display title for this navigation item.
    pub fn title(&self) -> String {
        match self {
            Self::SystemTimeline => tr("Activity Timeline"),
            Self::SystemHealth => tr("System Health"),
            Self::Performance => tr("Performance"),
            Self::Security => tr("Security Posture"),
            Self::ServicesManager => tr("Services Manager"),
            Self::Snapshots => tr("Known-Good Snapshot"),
            Self::Settings => tr("Settings"),
            Self::Help => tr("Help"),
        }
    }

    /// Returns all navigation items.
    pub fn all() -> &'static [NavItem] {
        &[
            Self::SystemHealth,
            Self::Performance,
            Self::Security,
            Self::SystemTimeline,
            Self::ServicesManager,
            Self::Snapshots,
            Self::Settings,
            Self::Help,
        ]
    }
}

/// The main application window.
pub struct MainWindow;

impl MainWindow {
    /// Creates a new main window with sidebar navigation.
    #[must_use]
    pub fn new(app: &adw::Application) -> adw::ApplicationWindow {
        debug!("Creating main window");


        let state = create_shared_state();


        {
            let desktop_info = detect_desktop();
            debug!(
                environment = ?desktop_info.environment,
                session = ?desktop_info.session_type,
                "Desktop detected"
            );
            if let Ok(mut s) = state.write() {
                s.desktop_info = Some(desktop_info);
            }
        }


        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(1280)
            .default_height(860)
            .build();


        let timeline_container = Rc::new(RefCell::new(None::<gtk4::Box>));
        let content_title = Rc::new(RefCell::new(None::<adw::WindowTitle>));


        let timeline_loaded = Rc::new(Cell::new(false));
        let performance_loaded = Rc::new(Cell::new(false));
        let security_loaded = Rc::new(Cell::new(false));
        let services_loaded = Rc::new(Cell::new(false));
        let snapshots_loaded = Rc::new(Cell::new(false));
        let settings_loaded = Rc::new(Cell::new(false));
        let help_loaded = Rc::new(Cell::new(false));


        let toast_overlay = Self::build_layout(
            &state,
            &window,
            timeline_container.clone(),
            content_title.clone(),
            timeline_loaded.clone(),
            performance_loaded.clone(),
            security_loaded.clone(),
            services_loaded.clone(),
            snapshots_loaded.clone(),
            settings_loaded.clone(),
            help_loaded.clone(),
        );
        window.set_content(Some(&toast_overlay));


        Self::setup_window_actions(
            &window,
            state.clone(),
            timeline_container.clone(),
            toast_overlay.clone(),
        );

        window
    }

    /// Builds the main layout with sidebar and content area.
    fn build_layout(
        state: &SharedState,
        window: &adw::ApplicationWindow,
        timeline_container: Rc<RefCell<Option<gtk4::Box>>>,
        content_title_ref: Rc<RefCell<Option<adw::WindowTitle>>>,
        timeline_loaded: Rc<Cell<bool>>,
        performance_loaded: Rc<Cell<bool>>,
        security_loaded: Rc<Cell<bool>>,
        services_loaded: Rc<Cell<bool>>,
        snapshots_loaded: Rc<Cell<bool>>,
        settings_loaded: Rc<Cell<bool>>,
        help_loaded: Rc<Cell<bool>>,
    ) -> adw::ToastOverlay {

        let main_box = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);


        let sidebar_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        sidebar_box.set_width_request(250); // 250px when expanded
        sidebar_box.add_css_class("sidebar-container");


        let sidebar_header = adw::HeaderBar::new();
        sidebar_header.set_show_end_title_buttons(false);
        sidebar_header.set_show_start_title_buttons(false);


        let sidebar_toggle_btn = gtk4::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text("Collapse sidebar")
            .build();
        sidebar_toggle_btn.add_css_class("flat");
        sidebar_header.pack_end(&sidebar_toggle_btn);

        let sidebar_title = adw::WindowTitle::new(APP_NAME, "");
        sidebar_header.set_title_widget(Some(&sidebar_title));
        sidebar_box.append(&sidebar_header);


        let sidebar_list = gtk4::ListBox::new();
        sidebar_list.set_selection_mode(gtk4::SelectionMode::Single);
        sidebar_list.add_css_class("navigation-sidebar");


        let nav_labels: Rc<RefCell<Vec<gtk4::Label>>> = Rc::new(RefCell::new(Vec::new()));
        let nav_boxes: Rc<RefCell<Vec<gtk4::Box>>> = Rc::new(RefCell::new(Vec::new()));

        for nav_item in NavItem::all() {
            let (row, label, hbox) = Self::create_nav_row_with_label(*nav_item);
            sidebar_list.append(&row);
            nav_labels.borrow_mut().push(label);
            nav_boxes.borrow_mut().push(hbox);
        }

        let sidebar_scroll = gtk4::ScrolledWindow::new();
        sidebar_scroll.set_vexpand(true);
        sidebar_scroll.set_child(Some(&sidebar_list));
        sidebar_box.append(&sidebar_scroll);


        let info_box = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
        info_box.set_margin_start(12);
        info_box.set_margin_end(12);
        info_box.set_margin_top(8);
        info_box.set_margin_bottom(8);


        let update_banner = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        update_banner.add_css_class("update-banner");
        update_banner.set_visible(false);
        update_banner.set_halign(gtk4::Align::Start);
        info_box.append(&update_banner);

        let version_label = gtk4::Label::new(None);
        version_label.set_markup(&format!("<span size=\"x-small\">Version {}</span>", APP_VERSION));
        version_label.set_halign(gtk4::Align::Start);
        version_label.add_css_class("dim-label");
        info_box.append(&version_label);

        sidebar_box.append(&info_box);


        Self::check_for_updates(update_banner);


        let separator = gtk4::Separator::new(gtk4::Orientation::Vertical);

        main_box.append(&sidebar_box);
        main_box.append(&separator);


        let sidebar_collapsed = Rc::new(Cell::new(false));
        let sidebar_box_ref = sidebar_box.clone();
        let sidebar_title_ref = sidebar_title.clone();
        let info_box_ref = info_box.clone();
        let nav_labels_ref = nav_labels.clone();
        let nav_boxes_ref = nav_boxes.clone();
        let btn_ref = sidebar_toggle_btn.clone();
        let collapsed_ref = sidebar_collapsed.clone();

        sidebar_toggle_btn.connect_clicked(move |_| {
            let is_collapsed = collapsed_ref.get();
            let new_collapsed = !is_collapsed;
            collapsed_ref.set(new_collapsed);


            if new_collapsed {
                sidebar_box_ref.set_width_request(50);
                sidebar_box_ref.add_css_class("sidebar-collapsed");
            } else {
                sidebar_box_ref.set_width_request(250);
                sidebar_box_ref.remove_css_class("sidebar-collapsed");
            }


            sidebar_title_ref.set_visible(!new_collapsed);


            for label in nav_labels_ref.borrow().iter() {
                label.set_visible(!new_collapsed);
            }
            for hbox in nav_boxes_ref.borrow().iter() {
                if new_collapsed {
                    hbox.set_margin_start(0);
                    hbox.set_margin_end(0);
                    hbox.set_spacing(0);
                    hbox.set_halign(gtk4::Align::Center);
                } else {
                    hbox.set_margin_start(12);
                    hbox.set_margin_end(12);
                    hbox.set_spacing(12);
                    hbox.set_halign(gtk4::Align::Fill);
                }
            }


            info_box_ref.set_visible(!new_collapsed);


            if new_collapsed {
                btn_ref.set_tooltip_text(Some(&tr("Expand sidebar")));
                btn_ref.set_icon_name("sidebar-show-right-symbolic");
            } else {
                btn_ref.set_tooltip_text(Some(&tr("Collapse sidebar")));
                btn_ref.set_icon_name("sidebar-show-symbolic");
            }
        });

        // Register toggle-sidebar action so keyboard shortcut (F9) works
        let toggle_btn_for_action = sidebar_toggle_btn.clone();
        let toggle_action = gio::SimpleAction::new("toggle-sidebar", None);
        toggle_action.connect_activate(move |_, _| {
            toggle_btn_for_action.emit_clicked();
        });
        window.add_action(&toggle_action);


        let content_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content_box.set_hexpand(true);


        let header_bar = adw::HeaderBar::new();
        let content_title = adw::WindowTitle::new("System Health", "");
        header_bar.set_title_widget(Some(&content_title));
        *content_title_ref.borrow_mut() = Some(content_title);


        let refresh_button = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_button.set_tooltip_text(Some(&tr("Refresh events")));
        refresh_button.add_css_class("flat");
        refresh_button.set_action_name(Some("win.refresh"));
        header_bar.pack_start(&refresh_button);


        let menu_button = gtk4::MenuButton::new();
        menu_button.set_icon_name("open-menu-symbolic");
        let theme_popover = ThemePopover::new();
        menu_button.set_popover(Some(&theme_popover));
        header_bar.pack_end(&menu_button);

        content_box.append(&header_bar);


        let content_stack = gtk4::Stack::new();
        content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        content_stack.set_transition_duration(200);
        content_stack.set_vexpand(true);
        content_stack.set_hexpand(true);

        // System Health page (main page - loads immediately)
        let health_page = SystemHealthPage::new();
        content_stack.add_named(&health_page, Some("health"));

        // Timeline page placeholder (lazy loaded)
        let timeline_placeholder = Self::build_loading_placeholder("System Activity Timeline");
        content_stack.add_named(&timeline_placeholder, Some("timeline"));

        // Performance page placeholder (lazy loaded)
        let performance_placeholder = Self::build_loading_placeholder("Performance");
        content_stack.add_named(&performance_placeholder, Some("performance"));

        // Security page placeholder (lazy loaded)
        let security_placeholder = Self::build_loading_placeholder("Security Posture");
        content_stack.add_named(&security_placeholder, Some("security"));

        // Services page placeholder (lazy loaded)
        let services_placeholder = Self::build_loading_placeholder("Services Manager");
        content_stack.add_named(&services_placeholder, Some("services"));

        // Snapshot page placeholder (lazy loaded)
        let snapshot_placeholder = Self::build_loading_placeholder("Known-Good Snapshot");
        content_stack.add_named(&snapshot_placeholder, Some("snapshots"));

        // Settings page placeholder (lazy loaded)
        let settings_placeholder = Self::build_loading_placeholder("Settings");
        content_stack.add_named(&settings_placeholder, Some("settings"));

        // Help page placeholder (lazy loaded)
        let help_placeholder = Self::build_loading_placeholder("Help");
        content_stack.add_named(&help_placeholder, Some("help"));

        content_box.append(&content_stack);

        main_box.append(&content_box);

        // Set up navigation with lazy loading
        let stack_clone = content_stack.clone();
        let title_ref = content_title_ref.clone();
        let state_clone = state.clone();
        let timeline_container_clone = timeline_container.clone();
        sidebar_list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let index = row.index() as usize;
                if let Some(nav_item) = NavItem::all().get(index) {

                    let view_name = match nav_item {
                        NavItem::SystemTimeline => "timeline",
                        NavItem::SystemHealth => "health",
                        NavItem::Performance => "performance",
                        NavItem::Security => "security",
                        NavItem::ServicesManager => "services",
                        NavItem::Snapshots => "snapshots",
                        NavItem::Settings => "settings",
                        NavItem::Help => "help",
                    };


                    match nav_item {
                        NavItem::SystemTimeline => {
                            if !timeline_loaded.get() {
                                timeline_loaded.set(true);
                                debug!("Lazy loading Timeline page");
                                
                                if let Some(placeholder) = stack_clone.child_by_name("timeline") {
                                    stack_clone.remove(&placeholder);
                                }
                                
                                let timeline_page = Self::build_timeline_page(&state_clone, timeline_container_clone.clone());
                                stack_clone.add_named(&timeline_page, Some("timeline"));
                                
                                Self::load_events(&state_clone, timeline_container_clone.clone(), None);
                            }
                        }
                        NavItem::Performance => {
                            if !performance_loaded.get() {
                                performance_loaded.set(true);
                                debug!("Lazy loading Performance page");
                                
                                if let Some(placeholder) = stack_clone.child_by_name("performance") {
                                    stack_clone.remove(&placeholder);
                                }
                                
                                let performance_page = PerformancePage::new();
                                stack_clone.add_named(&performance_page, Some("performance"));
                            }
                        }
                        NavItem::Security => {
                            if !security_loaded.get() {
                                security_loaded.set(true);
                                debug!("Lazy loading Security page");

                                if let Some(placeholder) = stack_clone.child_by_name("security") {
                                    stack_clone.remove(&placeholder);
                                }

                                let security_page = SecurityPage::new();
                                stack_clone.add_named(&security_page, Some("security"));
                            }
                        }
                        NavItem::ServicesManager => {
                            if !services_loaded.get() {
                                services_loaded.set(true);
                                debug!("Lazy loading Services page");
                                
                                if let Some(placeholder) = stack_clone.child_by_name("services") {
                                    stack_clone.remove(&placeholder);
                                }
                                
                                let services_page = ServicesPage::new();
                                stack_clone.add_named(&services_page, Some("services"));
                            }
                        }
                        NavItem::Snapshots => {
                            if !snapshots_loaded.get() {
                                snapshots_loaded.set(true);
                                debug!("Lazy loading Snapshot page");
                                
                                if let Some(placeholder) = stack_clone.child_by_name("snapshots") {
                                    stack_clone.remove(&placeholder);
                                }
                                
                                let snapshot_page = create_snapshot_page();
                                stack_clone.add_named(&snapshot_page, Some("snapshots"));
                            }
                        }
                        NavItem::Help => {
                            if !help_loaded.get() {
                                help_loaded.set(true);
                                debug!("Lazy loading Help page");
                                
                                if let Some(placeholder) = stack_clone.child_by_name("help") {
                                    stack_clone.remove(&placeholder);
                                }
                                
                                let help_page = create_help_page();
                                stack_clone.add_named(&help_page, Some("help"));
                            }
                        }
                        NavItem::Settings => {
                            if !settings_loaded.get() {
                                settings_loaded.set(true);
                                debug!("Lazy loading Settings page");
                                
                                if let Some(placeholder) = stack_clone.child_by_name("settings") {
                                    stack_clone.remove(&placeholder);
                                }
                                
                                let settings_page = create_settings_page();
                                let settings_scroll = gtk4::ScrolledWindow::builder()
                                    .hscrollbar_policy(gtk4::PolicyType::Never)
                                    .vscrollbar_policy(gtk4::PolicyType::Automatic)
                                    .vexpand(true)
                                    .build();
                                settings_scroll.set_child(Some(&settings_page));
                                stack_clone.add_named(&settings_scroll, Some("settings"));
                            }
                        }
                        NavItem::SystemHealth => {
                            // Already loaded at startup
                        }
                    }

                    stack_clone.set_visible_child_name(view_name);


                    if let Some(title) = title_ref.borrow().as_ref() {
                        title.set_title(&nav_item.title());
                    }
                }
            }
        });


        if let Some(first_row) = sidebar_list.row_at_index(0) {
            sidebar_list.select_row(Some(&first_row));
        }


        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&main_box));

        toast_overlay
    }

    /// Shows the export format chooser dialog.
    fn show_export_dialog(
        window: &adw::ApplicationWindow,
        state: &SharedState,
        toast_overlay: &adw::ToastOverlay,
    ) {
        use crate::application::use_cases::export::{export_events, ExportFormat};

        let events = state.read()
            .map(|s| s.filtered_events.clone())
            .unwrap_or_default();
        let groups = state.read()
            .map(|s| s.correlation_groups.clone())
            .unwrap_or_default();

        if events.is_empty() {
            let toast = adw::Toast::new("No events to export");
            toast.set_timeout(3);
            toast_overlay.add_toast(toast);
            return;
        }

        let dialog = adw::MessageDialog::new(
            Some(window),
            Some("Export Events"),
            Some(&format!("Export {} events to file", events.len())),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("json", "JSON");
        dialog.add_response("csv", "CSV");
        dialog.add_response("markdown", "Markdown");
        dialog.set_default_response(Some("json"));
        dialog.set_close_response("cancel");

        let toast_clone = toast_overlay.clone();
        let win_clone = window.clone();
        dialog.connect_response(None, move |dlg, response| {
            dlg.close();
            let format = match response {
                "json" => ExportFormat::Json,
                "csv" => ExportFormat::Csv,
                "markdown" => ExportFormat::Markdown,
                _ => return,
            };

            match export_events(&events, &groups, format) {
                Ok(content) => {

                    let file_dialog = gtk4::FileDialog::builder()
                        .title("Save Export")
                        .initial_name(format!(
                            "control-center-export.{}",
                            format.extension()
                        ))
                        .build();

                    let toast_save = toast_clone.clone();
                    let content_clone = content;
                    file_dialog.save(
                        Some(&win_clone),
                        Some(&gio::Cancellable::new()),
                        move |result| {
                            if let Ok(file) = result {
                                if let Some(path) = file.path() {
                                    match std::fs::write(&path, &content_clone) {
                                        Ok(()) => {
                                            let toast = adw::Toast::new(&format!(
                                                "Exported to {}",
                                                path.display()
                                            ));
                                            toast.set_timeout(3);
                                            toast_save.add_toast(toast);
                                        }
                                        Err(e) => {
                                            let toast = adw::Toast::new(&format!(
                                                "Export failed: {}",
                                                e
                                            ));
                                            toast.set_timeout(5);
                                            toast_save.add_toast(toast);
                                        }
                                    }
                                }
                            }
                        },
                    );
                }
                Err(e) => {
                    let toast = adw::Toast::new(&format!("Export failed: {}", e));
                    toast.set_timeout(5);
                    toast_clone.add_toast(toast);
                }
            }
        });

        dialog.present();
    }

    /// Creates a navigation row with icon and label for the sidebar.
    fn create_nav_row_with_label(nav_item: NavItem) -> (gtk4::ListBoxRow, gtk4::Label, gtk4::Box) {
        let row = gtk4::ListBoxRow::new();
        row.set_selectable(true);
        row.set_tooltip_text(Some(&nav_item.title()));

        let hbox = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);
        hbox.add_css_class("nav-row-box");

        let icon = gtk4::Image::from_icon_name(nav_item.icon_name());
        icon.set_pixel_size(20);
        hbox.append(&icon);

        let label = gtk4::Label::new(Some(&nav_item.title()));
        label.set_halign(gtk4::Align::Start);
        label.set_hexpand(true);
        label.add_css_class("nav-label");
        hbox.append(&label);


        row.update_property(&[
            gtk4::accessible::Property::Label(&format!("Navigate to {}", nav_item.title())),
        ]);

        row.set_child(Some(&hbox));
        (row, label, hbox)
    }

    /// Builds the System Activity Timeline page with filter sub-menu.
    fn build_timeline_page(state: &SharedState, timeline_container: Rc<RefCell<Option<gtk4::Box>>>) -> gtk4::Box {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 0);


        let filter_bar = FilterBar::new(state);
        page.append(&filter_bar);


        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let timeline = TimelineView::new(state);
        *timeline_container.borrow_mut() = Some(timeline.clone());
        scrolled.set_child(Some(&timeline));

        page.append(&scrolled);
        page.add_css_class("timeline-page");

        page
    }

    /// Builds a loading placeholder page shown while lazy loading.
    fn build_loading_placeholder(title: &str) -> gtk4::Box {
        let page = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        page.set_halign(gtk4::Align::Center);
        page.set_valign(gtk4::Align::Center);
        page.set_vexpand(true);

        let spinner = gtk4::Spinner::new();
        spinner.set_spinning(true);
        spinner.set_width_request(48);
        spinner.set_height_request(48);
        page.append(&spinner);

        let label = gtk4::Label::builder()
            .label(&format!("Loading {}...", title))
            .css_classes(vec!["dim-label".to_string()])
            .build();
        page.append(&label);

        page
    }

    /// Runs the one-time GitHub release check in a background thread.
    ///
    /// If a newer version is found, the hidden `update_banner` widget is
    /// populated with an icon and a `LinkButton` pointing to the release
    /// page, then made visible.  On any error the banner stays hidden.
    fn check_for_updates(update_banner: gtk4::Box) {
        use std::sync::{Arc, Mutex};

        // Shared slot: the background thread writes the result, the
        // main-thread timer reads it.  `Option<Option<UpdateInfo>>`:
        //   `None`          – not finished yet
        //   `Some(None)`    – finished, no update
        //   `Some(Some(i))` – finished, update available
        let slot: Arc<Mutex<Option<Option<crate::version_check::UpdateInfo>>>> =
            Arc::new(Mutex::new(None));
        let slot_bg = slot.clone();

        std::thread::spawn(move || {
            let result = crate::version_check::check_for_update(APP_VERSION);
            if let Ok(mut guard) = slot_bg.lock() {
                *guard = Some(result);
            }
        });

        // Poll from the main thread until the result arrives.
        glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
            let ready = slot.lock().ok().and_then(|guard| guard.clone());
            match ready {
                Some(Some(info)) => {
                    let icon = gtk4::Image::from_icon_name(
                        "software-update-available-symbolic",
                    );
                    icon.set_pixel_size(14);
                    icon.add_css_class("update-icon");
                    update_banner.append(&icon);

                    let label_text = format!("v{} available", info.latest_version);
                    let link = gtk4::LinkButton::with_label(
                        &info.download_url,
                        &label_text,
                    );
                    link.add_css_class("update-link");
                    update_banner.append(&link);

                    update_banner.set_visible(true);

                    debug!(
                        version = %info.latest_version,
                        url = %info.download_url,
                        "Update available — banner shown"
                    );
                    glib::ControlFlow::Break
                }
                Some(None) => {
                    // Check done, no update — stop polling.
                    glib::ControlFlow::Break
                }
                None => {
                    // Still waiting.
                    glib::ControlFlow::Continue
                }
            }
        });
    }

    /// Loads events from all available adapters using a background thread
    /// with state-based polling for UI updates.
    fn load_events(state: &SharedState, timeline_container: Rc<RefCell<Option<gtk4::Box>>>, toast_overlay: Option<adw::ToastOverlay>) {
        info!("Loading events...");


        if let Ok(mut s) = state.write() {
            s.is_loading = true;
            s.error = None;
        }

        let state_clone = state.clone();


        std::thread::spawn(move || {
            let services = create_services();
            let since = Utc::now() - chrono::Duration::hours(2);

            match services.ingestion.ingest_all(since) {
                Ok(mut events) => {
                    if events.len() > 500 {
                        events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                        events.truncate(500);
                    }
                    
                    info!(count = events.len(), "Events loaded successfully");
                    let groups = services.correlation.correlate(&events);

                    if let Ok(mut s) = state_clone.write() {
                        s.set_events(events);
                        s.correlation_groups = groups;
                        s.is_loading = false;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Failed to load events");
                    if let Ok(mut s) = state_clone.write() {
                        s.is_loading = false;
                        s.error = Some(format!("Failed to load events: {}", e));
                    }
                }
            }
        });


        let state_for_poll = state.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            let is_loading = state_for_poll.read().map(|s| s.is_loading).unwrap_or(true);

            if !is_loading {

                let error_msg = state_for_poll.read()
                    .ok()
                    .and_then(|s| s.error.clone());

                if let Some(err) = error_msg {
                    error!(error = %err, "Event loading failed");
                    if let Some(ref overlay) = toast_overlay {
                        let toast = adw::Toast::new(&format!("Error: {}", err));
                        toast.set_timeout(5);
                        overlay.add_toast(toast);
                    }
                } else {

                    let events = state_for_poll.read()
                        .map(|s| s.filtered_events.clone())
                        .unwrap_or_default();

                    debug!(event_count = events.len(), "Load complete, updating UI");

                    if let Some(container) = timeline_container.borrow().as_ref() {
                        TimelineView::update_events(container, &events);

                        if let Some(ref overlay) = toast_overlay {
                            let toast = adw::Toast::new(&format!("{} events loaded", events.len()));
                            toast.set_timeout(2);
                            overlay.add_toast(toast);
                        }
                    }

                    // Run data retention in background on first successful load
                    let config = state_for_poll.read()
                        .map(|s| s.config.clone())
                        .unwrap_or_default();
                    if config.data_retention_days > 0 {
                        std::thread::spawn(move || {
                            crate::infrastructure::storage::retention::run_retention(
                                config.data_retention_days,
                                0,
                            );
                        });
                    }
                }
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        });
    }

    /// Sets up window actions.
    fn setup_window_actions(
        window: &adw::ApplicationWindow,
        state: SharedState,
        timeline_container: Rc<RefCell<Option<gtk4::Box>>>,
        toast_overlay: adw::ToastOverlay,
    ) {

        let state_for_refresh = state.clone();
        let container_for_refresh = timeline_container.clone();
        let toast_for_refresh = toast_overlay.clone();
        let refresh_action = gio::SimpleAction::new("refresh", None);
        refresh_action.connect_activate(move |_, _| {
            debug!("Refresh action triggered");
            Self::load_events(&state_for_refresh, container_for_refresh.clone(), Some(toast_for_refresh.clone()));
        });
        window.add_action(&refresh_action);


        let state_for_ui = state.clone();
        let container_for_ui = timeline_container.clone();
        let refresh_ui_action = gio::SimpleAction::new("refresh-ui", None);
        refresh_ui_action.connect_activate(move |_, _| {
            debug!("Refresh UI action triggered");

            let events = state_for_ui.read()
                .map(|s| s.filtered_events.clone())
                .unwrap_or_default();

            debug!(event_count = events.len(), "Updating UI with filtered events");

            if let Some(container) = container_for_ui.borrow().as_ref() {
                TimelineView::update_events(container, &events);
            }
        });
        window.add_action(&refresh_ui_action);


        let state_for_export = state.clone();
        let window_for_export = window.clone();
        let toast_for_export = toast_overlay;
        let export_action = gio::SimpleAction::new("export", None);
        export_action.connect_activate(move |_, _| {
            debug!("Export action triggered");
            Self::show_export_dialog(&window_for_export, &state_for_export, &toast_for_export);
        });
        window.add_action(&export_action);


        let focus_search_action = gio::SimpleAction::new("focus-search", None);
        focus_search_action.connect_activate(move |_, _| {
            debug!("Focus search action triggered");
        });
        window.add_action(&focus_search_action);
    }
}
