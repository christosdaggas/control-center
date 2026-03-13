//! Timeline page - Main event view.

use crate::application::state::SharedState;
use crate::ui::widgets::TimelineView;
use gtk4::prelude::*;
use gtk4::ScrolledWindow;

/// The main timeline page showing system events.
pub struct TimelinePage;

impl TimelinePage {
    /// Creates a new timeline page.
    /// Returns (page_widget, timeline_container) where timeline_container can be updated.
    #[must_use]
    pub fn new(state: &SharedState) -> (gtk4::Box, gtk4::Box) {
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);

        // Create scrolled window for the timeline
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        // Create the timeline view
        let timeline = TimelineView::new(state);
        scrolled.set_child(Some(&timeline));

        container.append(&scrolled);
        container.add_css_class("timeline-view");

        (container, timeline)
    }
}
