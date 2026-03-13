//! Event row widget.

use crate::domain::event::Event;
use crate::i18n::tr;
use crate::ui::widgets::SeverityBadge;
use chrono::{DateTime, Local};
use gtk4::prelude::*;
use gtk4::{Box, Image, Label};
use gtk4::glib;
use gtk4::gdk;
use libadwaita as adw;
use libadwaita::prelude::*;

/// A row displaying a single event in the timeline.
pub struct EventRow;

impl EventRow {
    /// Creates a new event row.
    #[must_use]
    pub fn new(event: &Event) -> adw::ActionRow {

        let escaped_summary = glib::markup_escape_text(&event.summary);
        
        let row = adw::ActionRow::builder()
            .title(escaped_summary.as_str())
            .css_classes(vec!["event-row".to_string()])
            .build();


        let time: DateTime<Local> = event.timestamp.into();
        let subtitle = format!("{} · {}", time.format("%H:%M:%S"), event.event_type.label());
        row.set_subtitle(&subtitle);


        let icon = Image::from_icon_name(event.event_type.icon_name());
        icon.add_css_class(event.severity.css_class());
        row.add_prefix(&icon);


        let badge = SeverityBadge::new(event.severity);
        row.add_suffix(&badge);


        let copy_btn = gtk4::Button::from_icon_name("edit-copy-symbolic");
        copy_btn.set_tooltip_text(Some(&tr("Copy event message")));
        copy_btn.add_css_class("flat");
        copy_btn.set_valign(gtk4::Align::Center);
        

        let summary_for_copy = event.summary.clone();
        copy_btn.connect_clicked(move |_btn| {
            if let Some(display) = gdk::Display::default() {
                let clipboard = display.clipboard();
                clipboard.set_text(&summary_for_copy);
            }
        });
        row.add_suffix(&copy_btn);


        let chevron = Image::from_icon_name("go-next-symbolic");
        chevron.set_opacity(0.5);
        row.add_suffix(&chevron);

        row.set_activatable(true);


        row.update_property(&[
            gtk4::accessible::Property::Label(&format!(
                "{} {} event: {}",
                event.severity.label(),
                event.event_type.label(),
                event.summary
            )),
        ]);

        row
    }

    /// Creates an expanded detail view for an event.
    #[must_use]
    pub fn create_detail_view(event: &Event) -> gtk4::Box {
        let container = Box::new(gtk4::Orientation::Vertical, 12);
        container.add_css_class("detail-pane");


        let header = Label::builder()
            .label(&event.summary)
            .css_classes(vec!["detail-header".to_string()])
            .halign(gtk4::Align::Start)
            .build();
        container.append(&header);


        let meta_box = Box::new(gtk4::Orientation::Horizontal, 16);

        let time_label = Self::create_meta_label(
            "Time",
            &DateTime::<Local>::from(event.timestamp).format("%Y-%m-%d %H:%M:%S").to_string(),
        );
        meta_box.append(&time_label);

        let type_label = Self::create_meta_label("Type", event.event_type.label());
        meta_box.append(&type_label);

        if let Some(service) = &event.service {
            let svc_label = Self::create_meta_label("Service", service);
            meta_box.append(&svc_label);
        }

        if let Some(package) = &event.package {
            let pkg_label = Self::create_meta_label("Package", package);
            meta_box.append(&pkg_label);
        }

        container.append(&meta_box);


        if let Some(details) = &event.details {
            let details_label = Label::builder()
                .label(details)
                .wrap(true)
                .halign(gtk4::Align::Start)
                .css_classes(vec!["event-details".to_string()])
                .build();
            container.append(&details_label);
        }


        if !event.evidence.is_empty() {
            let evidence_header = Label::builder()
                .label(&tr("Source Evidence"))
                .css_classes(vec!["heading".to_string()])
                .halign(gtk4::Align::Start)
                .margin_top(12)
                .build();
            container.append(&evidence_header);

            for evidence in &event.evidence {
                let evidence_box = Box::new(gtk4::Orientation::Vertical, 4);
                evidence_box.add_css_class("evidence-box");

                let raw_label = Label::builder()
                    .label(&evidence.raw_content)
                    .wrap(true)
                    .halign(gtk4::Align::Start)
                    .selectable(true)
                    .build();
                evidence_box.append(&raw_label);

                container.append(&evidence_box);
            }
        }

        container
    }

    fn create_meta_label(title: &str, value: &str) -> gtk4::Box {
        let container = Box::new(gtk4::Orientation::Vertical, 2);

        let title_label = Label::builder()
            .label(title)
            .css_classes(vec!["dim-label".to_string()])
            .halign(gtk4::Align::Start)
            .build();

        let value_label = Label::builder()
            .label(value)
            .halign(gtk4::Align::Start)
            .build();

        container.append(&title_label);
        container.append(&value_label);

        container
    }
}
