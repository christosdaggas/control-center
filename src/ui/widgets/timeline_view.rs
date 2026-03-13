//! Timeline view widget.

use crate::application::state::SharedState;
use crate::domain::event::Event;
use crate::i18n::tr;
use crate::ui::widgets::EventRow;
use gtk4::prelude::*;
use gtk4::{Label, ListBox, SelectionMode};
use tracing::debug;

/// The main timeline view showing events in a list.
pub struct TimelineView;

impl TimelineView {
    /// Creates a new timeline view.
    #[must_use]
    pub fn new(state: &SharedState) -> gtk4::Box {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);


        let events = {
            state.read().ok().map(|s| s.filtered_events.clone()).unwrap_or_default()
        };

        if events.is_empty() {

            let empty = Self::create_empty_state();
            container.append(&empty);
        } else {

            let list_box = ListBox::builder()
                .selection_mode(SelectionMode::None)
                .css_classes(vec!["boxed-list".to_string()])
                .margin_start(16)
                .margin_end(16)
                .margin_top(16)
                .margin_bottom(16)
                .build();

            for event in &events {
                let row = EventRow::new(event);
                list_box.append(&row);
            }

            container.append(&list_box);
        }

        container
    }

    fn create_empty_state() -> gtk4::Box {
        let empty_box = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        empty_box.add_css_class("timeline-empty");
        empty_box.set_halign(gtk4::Align::Center);
        empty_box.set_valign(gtk4::Align::Center);
        empty_box.set_vexpand(true);

        let icon = gtk4::Image::from_icon_name("view-list-symbolic");
        icon.set_pixel_size(64);
        icon.set_opacity(0.5);
        empty_box.append(&icon);

        let title = Label::builder()
            .label(&tr("No Events"))
            .css_classes(vec!["title-2".to_string()])
            .build();
        empty_box.append(&title);

        let subtitle = Label::builder()
            .label(&tr("Click the refresh button to load system events"))
            .css_classes(vec!["dim-label".to_string()])
            .build();
        empty_box.append(&subtitle);


        let refresh_btn = gtk4::Button::builder()
            .label(&tr("Load Events"))
            .css_classes(vec!["suggested-action".to_string(), "pill".to_string()])
            .margin_top(12)
            .build();

        refresh_btn.set_action_name(Some("win.refresh"));

        empty_box.append(&refresh_btn);

        empty_box
    }

    /// Updates the timeline with new events.
    pub fn update_events(container: &gtk4::Box, events: &[Event]) {
        debug!(event_count = events.len(), "TimelineView::update_events called");
        

        while let Some(child) = container.first_child() {
            container.remove(&child);
        }
        
        debug!("Cleared existing children");

        if events.is_empty() {
            debug!("No events, showing empty state");
            let empty = Self::create_empty_state();
            container.append(&empty);
        } else {
            debug!(count = events.len(), "Creating list with events");
            let list_box = ListBox::builder()
                .selection_mode(SelectionMode::None)
                .css_classes(vec!["boxed-list".to_string()])
                .margin_start(16)
                .margin_end(16)
                .margin_top(16)
                .margin_bottom(16)
                .build();

            for event in events {
                let row = EventRow::new(event);
                list_box.append(&row);
            }

            container.append(&list_box);
            debug!("ListBox appended to container");
        }
    }
}
