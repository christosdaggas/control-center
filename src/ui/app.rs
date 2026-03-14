//! GTK4 Application setup.
//!
//! Handles application lifecycle and window management.

use crate::i18n::tr;
use crate::ui::window::MainWindow;
use gtk4::prelude::*;
use gtk4::gio;
use libadwaita as adw;
use libadwaita::prelude::*;
use tracing::{debug, info};

const APP_ID: &str = "com.chrisdaggas.control-center";
const APP_NAME: &str = "Control Center";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The main GTK application.
pub struct ControlCenterApp;

impl ControlCenterApp {
    /// Runs the application and returns the exit code.
    #[must_use]
    pub fn run() -> i32 {
        info!("Initializing GTK application");


        adw::init().expect("Failed to initialize libadwaita");


        let app = adw::Application::builder()
            .application_id(APP_ID)
            .flags(gio::ApplicationFlags::FLAGS_NONE)
            .build();


        app.connect_startup(Self::on_startup);
        app.connect_activate(Self::on_activate);


        Self::setup_actions(&app);


        debug!("Starting application main loop");
        app.run().into()
    }

    fn on_startup(_app: &adw::Application) {
        debug!("Application startup");


        if let Some(display) = gtk4::gdk::Display::default() {
            let icon_theme = gtk4::IconTheme::for_display(&display);
            

            if let Ok(exe_path) = std::env::current_exe() {
                if let Some(exe_dir) = exe_path.parent() {

                    let dev_icons = exe_dir.join("../../data/icons");
                    if dev_icons.exists() {
                        if let Some(path_str) = dev_icons.canonicalize().ok().and_then(|p| p.to_str().map(String::from)) {
                            icon_theme.add_search_path(&path_str);
                            debug!("Added icon search path: {}", path_str);
                        }
                    }
                }
            }
            

            icon_theme.add_search_path("data/icons");
        }


        gtk4::Window::set_default_icon_name(APP_ID);


        crate::ui::style::load_css();
    }

    fn on_activate(app: &adw::Application) {
        debug!("Application activate");


        let window = if let Some(window) = app.active_window() {
            window
        } else {
            let window = MainWindow::new(app);
            window.upcast()
        };

        window.present();
    }

    fn setup_actions(app: &adw::Application) {

        let about_action = gio::SimpleAction::new("about", None);
        let app_clone = app.clone();
        about_action.connect_activate(move |_, _| {
            Self::show_about_dialog(&app_clone);
        });
        app.add_action(&about_action);


        let help_action = gio::SimpleAction::new("show-help-overlay", None);
        let app_clone = app.clone();
        help_action.connect_activate(move |_, _| {
            Self::show_shortcuts_window(&app_clone);
        });
        app.add_action(&help_action);
        app.set_accels_for_action("app.show-help-overlay", &["<Control>question", "F1"]);


        let quit_action = gio::SimpleAction::new("quit", None);
        let app_clone = app.clone();
        quit_action.connect_activate(move |_, _| {
            app_clone.quit();
        });
        app.add_action(&quit_action);
        app.set_accels_for_action("app.quit", &["<Control>q"]);

        // ── Window-level keyboard shortcuts ─────────────────────────

        app.set_accels_for_action("win.refresh", &["<Control>r", "F5"]);


        app.set_accels_for_action("win.toggle-sidebar", &["F9"]);


        app.set_accels_for_action("win.focus-search", &["<Control>f"]);


        app.set_accels_for_action("win.export", &["<Control>e"]);
    }

    fn show_shortcuts_window(app: &adw::Application) {
        // In GTK4, ShortcutsWindow expects sections/groups to be added via UI definition
        // We'll create a simple help dialog with adw::MessageDialog instead
        let window = app.active_window();

        let dialog = adw::MessageDialog::new(
            window.as_ref(),
            Some(&tr("Keyboard Shortcuts")),
            None,
        );


        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_margin_top(6);
        content.set_margin_bottom(6);


        let nav_frame = gtk4::Frame::new(Some(&tr("Navigation")));
        let nav_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        nav_box.set_margin_start(12);
        nav_box.set_margin_end(12);
        nav_box.set_margin_top(8);
        nav_box.set_margin_bottom(8);

        for (action, shortcut) in [
            (&*tr("Switch pages"), "Alt + 1...4"),
            (&*tr("Toggle sidebar"), "F9"),
            (&*tr("Open menu"), "F10"),
            (&*tr("Quit application"), "Ctrl + Q"),
        ] {
            let row = Self::create_shortcut_row(action, shortcut);
            nav_box.append(&row);
        }
        nav_frame.set_child(Some(&nav_box));
        content.append(&nav_frame);


        let snap_frame = gtk4::Frame::new(Some(&tr("Snapshots")));
        let snap_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        snap_box.set_margin_start(12);
        snap_box.set_margin_end(12);
        snap_box.set_margin_top(8);
        snap_box.set_margin_bottom(8);

        for (action, shortcut) in [
            (&*tr("Create new snapshot"), "Ctrl + N"),
            (&*tr("Compare to current"), "Ctrl + Enter"),
            (&*tr("Delete snapshot"), "Delete"),
        ] {
            let row = Self::create_shortcut_row(action, shortcut);
            snap_box.append(&row);
        }
        snap_frame.set_child(Some(&snap_box));
        content.append(&snap_frame);


        let gen_frame = gtk4::Frame::new(Some(&tr("General")));
        let gen_box = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
        gen_box.set_margin_start(12);
        gen_box.set_margin_end(12);
        gen_box.set_margin_top(8);
        gen_box.set_margin_bottom(8);

        for (action, shortcut) in [
            (&*tr("Show keyboard shortcuts"), "Ctrl + ? or F1"),
            (&*tr("Refresh events"), "Ctrl + R or F5"),
            (&*tr("Search / filter"), "Ctrl + F"),
            (&*tr("Export events"), "Ctrl + E"),
        ] {
            let row = Self::create_shortcut_row(action, shortcut);
            gen_box.append(&row);
        }
        gen_frame.set_child(Some(&gen_box));
        content.append(&gen_frame);

        dialog.set_extra_child(Some(&content));
        dialog.add_response("close", &tr("Close"));
        dialog.set_default_response(Some("close"));
        dialog.present();
    }

    fn create_shortcut_row(action: &str, shortcut: &str) -> gtk4::Box {
        let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);

        let label = gtk4::Label::new(Some(action));
        label.set_halign(gtk4::Align::Start);
        label.set_hexpand(true);
        row.append(&label);

        let accel = gtk4::Label::new(Some(shortcut));
        accel.add_css_class("dim-label");
        accel.add_css_class("monospace");
        row.append(&accel);

        row
    }

    fn show_about_dialog(app: &adw::Application) {
        let about = adw::AboutWindow::builder()
            .application_name(APP_NAME)
            .application_icon(APP_ID)
            .developer_name("Christos A. Daggas")
            .version(APP_VERSION)
            .license_type(gtk4::License::MitX11)
            .website("https://chrisdaggas.com")
            .issue_url("https://github.com/chrisdaggas/control-center/issues")
            .comments(&tr("A modern Linux system monitoring and control center.\n\nView system health, manage services, and monitor activity timeline."))
            .release_notes(
                "<p>Version 1.5.0</p>\
                 <ul>\
                 <li>New Security Posture page for firewall state, SELinux/AppArmor mode, Secure Boot, listening ports, SSH exposure, admin access, and Flatpak permissions</li>\
                 <li>Security posture is now captured in snapshots and highlighted in snapshot comparisons</li>\
                 <li>Improved security drift visibility with dedicated findings for exposure, policy enforcement, privileged access, and sandboxing</li>\
                 <li>Help and navigation updated to surface the new security tooling more clearly</li>\
                 </ul>"
            )
            .release_notes_version(APP_VERSION)
            .build();

        if let Some(window) = app.active_window() {
            about.set_transient_for(Some(&window));
        }
        about.present();
    }
}
