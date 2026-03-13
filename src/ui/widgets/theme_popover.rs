//! Theme Selector Widget - Popup with 3 circles for theme selection.


use crate::i18n::tr;
use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;

/// Theme popover with theme selector and menu items.
pub struct ThemePopover;

impl ThemePopover {
    /// Creates a new theme popover.
    #[must_use]
    pub fn new() -> gtk::Popover {
        let popover = gtk::Popover::new();
        popover.add_css_class("menu");

        let main_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(0)
            .width_request(280)
            .build();


        let theme_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(18)
            .halign(gtk::Align::Center)
            .margin_top(18)
            .margin_bottom(18)
            .build();


        let default_btn = gtk::ToggleButton::new();
        let light_btn = gtk::ToggleButton::new();
        let dark_btn = gtk::ToggleButton::new();


        fn create_theme_content(css_class: &str, is_selected: bool) -> gtk::Overlay {
            let overlay = gtk::Overlay::new();
            
            let icon = gtk::Box::builder()
                .width_request(44)
                .height_request(44)
                .build();
            icon.add_css_class("theme-selector");
            icon.add_css_class(css_class);
            overlay.set_child(Some(&icon));
            
            if is_selected {
                let check = gtk::Image::from_icon_name("object-select-symbolic");
                check.add_css_class("theme-check");
                check.set_halign(gtk::Align::Center);
                check.set_valign(gtk::Align::Center);
                overlay.add_overlay(&check);
            }
            
            overlay
        }


        default_btn.set_child(Some(&create_theme_content("theme-default", false)));
        default_btn.set_tooltip_text(Some(&tr("System")));
        default_btn.add_css_class("flat");
        default_btn.add_css_class("circular");
        default_btn.add_css_class("theme-button");

        light_btn.set_child(Some(&create_theme_content("theme-light", false)));
        light_btn.set_tooltip_text(Some(&tr("Light")));
        light_btn.add_css_class("flat");
        light_btn.add_css_class("circular");
        light_btn.add_css_class("theme-button");

        dark_btn.set_child(Some(&create_theme_content("theme-dark", false)));
        dark_btn.set_tooltip_text(Some(&tr("Dark")));
        dark_btn.add_css_class("flat");
        dark_btn.add_css_class("circular");
        dark_btn.add_css_class("theme-button");


        light_btn.set_group(Some(&default_btn));
        dark_btn.set_group(Some(&default_btn));


        let style_manager = adw::StyleManager::default();
        
        match style_manager.color_scheme() {
            adw::ColorScheme::ForceLight => {
                light_btn.set_active(true);
                light_btn.set_child(Some(&create_theme_content("theme-light", true)));
            }
            adw::ColorScheme::ForceDark => {
                dark_btn.set_active(true);
                dark_btn.set_child(Some(&create_theme_content("theme-dark", true)));
            }
            _ => {
                default_btn.set_active(true);
                default_btn.set_child(Some(&create_theme_content("theme-default", true)));
            }
        }


        let light_btn_clone = light_btn.clone();
        let dark_btn_clone = dark_btn.clone();
        default_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::Default);
                btn.set_child(Some(&create_theme_content("theme-default", true)));
                light_btn_clone.set_child(Some(&create_theme_content("theme-light", false)));
                dark_btn_clone.set_child(Some(&create_theme_content("theme-dark", false)));
            }
        });

        let default_btn_clone = default_btn.clone();
        let dark_btn_clone2 = dark_btn.clone();
        light_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::ForceLight);
                btn.set_child(Some(&create_theme_content("theme-light", true)));
                default_btn_clone.set_child(Some(&create_theme_content("theme-default", false)));
                dark_btn_clone2.set_child(Some(&create_theme_content("theme-dark", false)));
            }
        });

        let default_btn_clone2 = default_btn.clone();
        let light_btn_clone2 = light_btn.clone();
        dark_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                let style_manager = adw::StyleManager::default();
                style_manager.set_color_scheme(adw::ColorScheme::ForceDark);
                btn.set_child(Some(&create_theme_content("theme-dark", true)));
                default_btn_clone2.set_child(Some(&create_theme_content("theme-default", false)));
                light_btn_clone2.set_child(Some(&create_theme_content("theme-light", false)));
            }
        });

        theme_box.append(&default_btn);
        theme_box.append(&light_btn);
        theme_box.append(&dark_btn);
        main_box.append(&theme_box);


        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        separator.set_margin_start(12);
        separator.set_margin_end(12);
        main_box.append(&separator);


        let menu_list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        menu_list.set_margin_top(6);
        menu_list.set_margin_bottom(6);
        menu_list.set_margin_start(6);
        menu_list.set_margin_end(6);


        let help_btn = gtk::Button::new();
        let help_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        help_box.set_margin_start(6);
        help_box.set_margin_end(6);
        help_box.set_margin_top(8);
        help_box.set_margin_bottom(8);
        let help_icon = gtk::Image::from_icon_name("preferences-desktop-keyboard-shortcuts-symbolic");
        let help_label = gtk::Label::new(Some(&tr("Keyboard Shortcuts")));
        help_label.set_halign(gtk::Align::Start);
        help_label.set_hexpand(true);
        help_box.append(&help_icon);
        help_box.append(&help_label);
        help_btn.set_child(Some(&help_box));
        help_btn.add_css_class("flat");
        help_btn.add_css_class("menu-item");
        help_btn.set_action_name(Some("app.show-help-overlay"));
        menu_list.append(&help_btn);


        let about_btn = gtk::Button::new();
        let about_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        about_box.set_margin_start(6);
        about_box.set_margin_end(6);
        about_box.set_margin_top(8);
        about_box.set_margin_bottom(8);
        let about_icon = gtk::Image::from_icon_name("help-about-symbolic");
        let about_label = gtk::Label::new(Some(&tr("About Control Center")));
        about_label.set_halign(gtk::Align::Start);
        about_label.set_hexpand(true);
        about_box.append(&about_icon);
        about_box.append(&about_label);
        about_btn.set_child(Some(&about_box));
        about_btn.add_css_class("flat");
        about_btn.add_css_class("menu-item");
        about_btn.set_action_name(Some("app.about"));
        menu_list.append(&about_btn);


        let quit_btn = gtk::Button::new();
        let quit_box = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        quit_box.set_margin_start(6);
        quit_box.set_margin_end(6);
        quit_box.set_margin_top(8);
        quit_box.set_margin_bottom(8);
        let quit_icon = gtk::Image::from_icon_name("application-exit-symbolic");
        let quit_label = gtk::Label::new(Some(&tr("Quit")));
        quit_label.set_halign(gtk::Align::Start);
        quit_label.set_hexpand(true);
        quit_box.append(&quit_icon);
        quit_box.append(&quit_label);
        quit_btn.set_child(Some(&quit_box));
        quit_btn.add_css_class("flat");
        quit_btn.add_css_class("menu-item");
        quit_btn.set_action_name(Some("app.quit"));
        menu_list.append(&quit_btn);

        main_box.append(&menu_list);

        popover.set_child(Some(&main_box));
        popover
    }

    /// Apply theme using adwaita style manager.
    pub fn apply_theme(theme: &str) {
        let style_manager = adw::StyleManager::default();
        match theme {
            "light" => style_manager.set_color_scheme(adw::ColorScheme::ForceLight),
            "dark" => style_manager.set_color_scheme(adw::ColorScheme::ForceDark),
            _ => style_manager.set_color_scheme(adw::ColorScheme::Default),
        }
    }
}
