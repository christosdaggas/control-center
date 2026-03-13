//! Known-Good Snapshot page.
//!
//! Allows users to create system state snapshots and compare them
//! to identify what changed since a known-good state.

use crate::application::use_cases::compare_snapshots;
use crate::domain::diff::{ChangeType, DiffCategory, DiffEntry, SnapshotDiff};
use crate::domain::snapshot::{Snapshot, SnapshotMetadata};
use crate::i18n::tr;
use crate::infrastructure::adapters::snapshot::CollectorRegistry;
use crate::infrastructure::storage::SnapshotStore;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Entry, Label, ListBox, ListBoxRow, Orientation,
    PolicyType, ScrolledWindow, SelectionMode, Separator,
};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use tracing::{debug, error, info};

/// Creates the Known-Good Snapshot page.
#[must_use]
pub fn create_snapshot_page() -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 0);

    let main_box = GtkBox::new(Orientation::Vertical, 16);
    main_box.set_margin_top(24);
    main_box.set_margin_bottom(24);
    main_box.set_margin_start(24);
    main_box.set_margin_end(24);
    main_box.set_vexpand(true);
    main_box.set_hexpand(true);

    // Header
    let header = create_header();
    main_box.append(&header);

    // Content panes
    let content_box = GtkBox::new(Orientation::Horizontal, 16);
    content_box.set_vexpand(true);
    content_box.set_hexpand(true);

    // State management
    let state = Rc::new(RefCell::new(SnapshotPageState::new()));

    // Left panel: Snapshot list
    let list_panel = create_snapshot_list_panel(state.clone());
    content_box.append(&list_panel);

    // Right panel: Actions and diff results
    let right_panel = create_right_panel(state.clone());
    right_panel.set_hexpand(true);
    content_box.append(&right_panel);

    main_box.append(&content_box);
    page.append(&main_box);

    // Load initial snapshot list
    refresh_snapshot_list(&state);

    page
}

/// State for the snapshot page.
struct SnapshotPageState {
    store: SnapshotStore,
    snapshots: Vec<SnapshotMetadata>,
    selected_snapshot: Option<uuid::Uuid>,
    current_diff: Option<SnapshotDiff>,
    list_box: Option<ListBox>,
    diff_container: Option<GtkBox>,
    compare_button: Option<Button>,
    delete_button: Option<Button>,
    status_label: Option<Label>,
    redact_mode: bool,
}

impl SnapshotPageState {
    fn new() -> Self {
        Self {
            store: SnapshotStore::new().expect("Failed to create snapshot store"),
            snapshots: Vec::new(),
            selected_snapshot: None,
            current_diff: None,
            list_box: None,
            diff_container: None,
            compare_button: None,
            delete_button: None,
            status_label: None,
            redact_mode: false,
        }
    }
}

/// Creates the page header.
fn create_header() -> GtkBox {
    let header = GtkBox::new(Orientation::Vertical, 8);

    let title = Label::new(Some(&tr("Known-Good Snapshot")));
    title.add_css_class("title-1");
    title.set_halign(gtk4::Align::Start);
    header.append(&title);

    let subtitle = Label::new(Some(
        &tr("Create snapshots of your system state and compare them to find what changed."),
    ));
    subtitle.add_css_class("dim-label");
    subtitle.set_halign(gtk4::Align::Start);
    subtitle.set_wrap(true);
    header.append(&subtitle);

    header
}

/// Creates the left panel with snapshot list.
fn create_snapshot_list_panel(state: Rc<RefCell<SnapshotPageState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 8);
    panel.set_width_request(320);
    panel.set_hexpand(false);
    panel.add_css_class("card");
    panel.set_margin_top(8);
    panel.set_margin_bottom(8);

    // Panel header
    let header_box = GtkBox::new(Orientation::Horizontal, 8);
    header_box.set_margin_start(12);
    header_box.set_margin_end(12);
    header_box.set_margin_top(12);

    let list_title = Label::new(Some(&tr("Snapshots")));
    list_title.add_css_class("heading");
    list_title.set_hexpand(true);
    list_title.set_halign(gtk4::Align::Start);
    header_box.append(&list_title);

    // Refresh button
    let refresh_btn = Button::from_icon_name("view-refresh-symbolic");
    refresh_btn.set_tooltip_text(Some(&tr("Refresh list")));
    let state_clone = state.clone();
    refresh_btn.connect_clicked(move |_| {
        refresh_snapshot_list(&state_clone);
    });
    header_box.append(&refresh_btn);

    panel.append(&header_box);
    panel.append(&Separator::new(Orientation::Horizontal));

    // Snapshot list
    let scrolled = ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);

    let list_box = ListBox::new();
    list_box.set_selection_mode(SelectionMode::Single);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(8);
    list_box.set_margin_end(8);
    list_box.set_margin_top(8);
    list_box.set_margin_bottom(8);

    // Handle selection
    let state_clone = state.clone();
    list_box.connect_row_selected(move |_, row| {
        let mut state = state_clone.borrow_mut();
        if let Some(row) = row {
            let index = row.index() as usize;
            if let Some(snapshot) = state.snapshots.get(index) {
                let snapshot_id = snapshot.id;
                state.selected_snapshot = Some(snapshot_id);
                if let Some(btn) = &state.compare_button {
                    btn.set_sensitive(true);
                }
                if let Some(btn) = &state.delete_button {
                    btn.set_sensitive(true);
                }
                debug!(id = %snapshot_id, "Selected snapshot");
            }
        } else {
            state.selected_snapshot = None;
            if let Some(btn) = &state.compare_button {
                btn.set_sensitive(false);
            }
            if let Some(btn) = &state.delete_button {
                btn.set_sensitive(false);
            }
        }
    });

    scrolled.set_child(Some(&list_box));
    panel.append(&scrolled);

    // Store reference
    state.borrow_mut().list_box = Some(list_box);

    // Create snapshot button
    let create_btn = Button::with_label(&tr("Create Snapshot"));
    create_btn.add_css_class("suggested-action");
    create_btn.set_margin_start(12);
    create_btn.set_margin_end(12);
    create_btn.set_margin_bottom(12);

    let state_clone = state.clone();
    create_btn.connect_clicked(move |btn| {
        show_create_snapshot_dialog(btn, state_clone.clone());
    });
    panel.append(&create_btn);

    panel
}

/// Creates the right panel with actions and diff results.
fn create_right_panel(state: Rc<RefCell<SnapshotPageState>>) -> GtkBox {
    let panel = GtkBox::new(Orientation::Vertical, 16);

    // Action buttons
    let action_box = GtkBox::new(Orientation::Horizontal, 8);

    // Compare button
    let compare_btn = Button::with_label(&tr("Compare to Current"));
    compare_btn.add_css_class("suggested-action");
    compare_btn.set_sensitive(false);
    let state_clone = state.clone();
    compare_btn.connect_clicked(move |_| {
        compare_selected_snapshot(&state_clone);
    });
    action_box.append(&compare_btn);

    // Delete button
    let delete_btn = Button::with_label(&tr("Delete"));
    delete_btn.add_css_class("destructive-action");
    delete_btn.set_sensitive(false);
    let state_clone = state.clone();
    delete_btn.connect_clicked(move |_| {
        delete_selected_snapshot(&state_clone);
    });
    action_box.append(&delete_btn);

    // Redact mode toggle
    let redact_box = GtkBox::new(Orientation::Horizontal, 8);
    redact_box.set_margin_start(16);
    redact_box.set_valign(gtk4::Align::Center);
    
    let redact_label = Label::new(Some(&tr("Redact sensitive data")));
    redact_box.append(&redact_label);
    
    let redact_switch = gtk4::Switch::new();
    redact_switch.set_valign(gtk4::Align::Center);
    let state_clone = state.clone();
    redact_switch.connect_state_set(move |_, active| {
        state_clone.borrow_mut().redact_mode = active;
        glib::Propagation::Proceed
    });
    redact_box.append(&redact_switch);
    action_box.append(&redact_box);

    // Spacer
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    action_box.append(&spacer);

    // Status label (with ellipsis to prevent resizing)
    let status_label = Label::new(None);
    status_label.add_css_class("dim-label");
    status_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    status_label.set_max_width_chars(30);
    action_box.append(&status_label);

    panel.append(&action_box);

    // Store button references
    {
        let mut s = state.borrow_mut();
        s.compare_button = Some(compare_btn);
        s.delete_button = Some(delete_btn);
        s.status_label = Some(status_label);
    }

    // Diff results container
    let diff_container = GtkBox::new(Orientation::Vertical, 0);
    diff_container.set_vexpand(true);

    // Initial placeholder
    let placeholder = create_diff_placeholder();
    diff_container.append(&placeholder);

    state.borrow_mut().diff_container = Some(diff_container.clone());
    panel.append(&diff_container);

    panel
}

/// Creates a placeholder for when no diff is shown.
fn create_diff_placeholder() -> adw::StatusPage {
    let status = adw::StatusPage::new();
    status.set_icon_name(Some("document-properties-symbolic"));
    status.set_title(&tr("Select a Snapshot"));
    status.set_description(Some(
        &tr("Select a snapshot from the list and click \"Compare to Current\" to see what changed."),
    ));
    status
}

/// Refreshes the snapshot list.
fn refresh_snapshot_list(state: &Rc<RefCell<SnapshotPageState>>) {
    // Get the list_box and store clones, then drop borrow
    let (list_box, snapshots) = {
        let s = state.borrow();
        let list_box = s.list_box.clone();
        let snapshots = match s.store.list() {
            Ok(snaps) => snaps,
            Err(e) => {
                error!(error = %e, "Failed to list snapshots");
                return;
            }
        };
        (list_box, snapshots)
    };

    // Update state with new snapshots
    state.borrow_mut().snapshots = snapshots.clone();

    // Now update UI without holding a borrow
    if let Some(list_box) = list_box {
        // Disconnect selection temporarily by setting selection mode
        list_box.set_selection_mode(SelectionMode::None);

        // Clear existing rows
        while let Some(child) = list_box.first_child() {
            list_box.remove(&child);
        }

        // Add snapshot rows
        for metadata in &snapshots {
            let row = create_snapshot_row(metadata);
            list_box.append(&row);
        }

        // Show empty state if no snapshots
        if snapshots.is_empty() {
            let empty_row = ListBoxRow::new();
            let empty_label = Label::new(Some(&tr("No snapshots yet")));
            empty_label.add_css_class("dim-label");
            empty_label.set_margin_top(24);
            empty_label.set_margin_bottom(24);
            empty_row.set_child(Some(&empty_label));
            empty_row.set_selectable(false);
            list_box.append(&empty_row);
        }

        // Restore selection mode
        list_box.set_selection_mode(SelectionMode::Single);

        debug!(count = snapshots.len(), "Refreshed snapshot list");
    }
}

/// Creates a row widget for a snapshot.
fn create_snapshot_row(metadata: &SnapshotMetadata) -> ListBoxRow {
    let row = ListBoxRow::new();

    let content = GtkBox::new(Orientation::Vertical, 4);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(8);
    content.set_margin_end(8);

    // Name
    let name_label = Label::new(Some(&metadata.name));
    name_label.add_css_class("heading");
    name_label.set_halign(gtk4::Align::Start);
    content.append(&name_label);

    // Details
    let details = format!(
        "{} • {} packages • {} services",
        metadata.created_at.format("%Y-%m-%d %H:%M"),
        metadata.package_count,
        metadata.system_unit_count
    );
    let details_label = Label::new(Some(&details));
    details_label.add_css_class("caption");
    details_label.add_css_class("dim-label");
    details_label.set_halign(gtk4::Align::Start);
    content.append(&details_label);

    // Redacted badge
    if metadata.redacted {
        let badge = Label::new(Some(&tr("Redacted")));
        badge.add_css_class("caption");
        badge.add_css_class("success");
        badge.set_halign(gtk4::Align::Start);
        content.append(&badge);
    }

    row.set_child(Some(&content));
    row
}

/// Shows the create snapshot dialog.
fn show_create_snapshot_dialog(button: &Button, state: Rc<RefCell<SnapshotPageState>>) {
    let window = button
        .root()
        .and_then(|r| r.downcast::<gtk4::Window>().ok());

    let dialog = adw::MessageDialog::new(
        window.as_ref(),
        Some(&tr("Create Snapshot")),
        Some(&tr("Enter a name for this snapshot.")),
    );

    // Add name entry
    let entry = Entry::new();
    entry.set_placeholder_text(Some(&tr("My Known-Good State")));
    entry.set_margin_start(24);
    entry.set_margin_end(24);
    dialog.set_extra_child(Some(&entry));

    dialog.add_response("cancel", &tr("Cancel"));
    dialog.add_response("create", &tr("Create"));
    dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("create"));

    let entry_clone = entry.clone();
    dialog.connect_response(None, move |dialog, response| {
        if response == "create" {
            let name = entry_clone.text().to_string();
            let name = if name.is_empty() {
                format!("Snapshot {}", chrono::Utc::now().format("%Y-%m-%d %H:%M"))
            } else {
                name
            };

            create_snapshot(&state, &name);
        }
        dialog.close();
    });

    dialog.present();
}

/// Creates a new snapshot.
fn create_snapshot(state: &Rc<RefCell<SnapshotPageState>>, name: &str) {
    info!(name = %name, "Creating snapshot");

    let redact = state.borrow().redact_mode;
    let mut snapshot = Snapshot::new(name).with_redaction(redact);

    // Collect data using all collectors
    let registry = CollectorRegistry::new();
    let errors = registry.collect_all(&mut snapshot, redact);

    if !errors.is_empty() {
        for e in &errors {
            error!(error = %e, "Collector error");
        }
    }

    // Save snapshot
    {
        let s = state.borrow();
        if let Err(e) = s.store.save(&snapshot) {
            error!(error = %e, "Failed to save snapshot");
            return;
        }
    }

    info!(
        id = %snapshot.id,
        packages = snapshot.packages.packages.len(),
        system_units = snapshot.systemd.system_units.len(),
        "Snapshot created"
    );

    // Refresh list
    refresh_snapshot_list(state);

    // Update status
    let s = state.borrow();
    if let Some(label) = &s.status_label {
        label.set_text(&format!(
            "Snapshot created ({} packages)",
            snapshot.packages.packages.len()
        ));
    }
}

/// Compares the selected snapshot to current system state.
fn compare_selected_snapshot(state: &Rc<RefCell<SnapshotPageState>>) {
    let (snapshot_id, redact) = {
        let s = state.borrow();
        match s.selected_snapshot {
            Some(id) => (id, s.redact_mode),
            None => return,
        }
    };

    info!(id = %snapshot_id, "Comparing snapshot to current state");

    // Load base snapshot
    let base_snapshot = {
        let s = state.borrow();
        match s.store.load(snapshot_id) {
            Ok(snap) => snap,
            Err(e) => {
                error!(error = %e, "Failed to load snapshot");
                return;
            }
        }
    };

    // Create current snapshot (in memory only)
    let mut current = Snapshot::new(&tr("Current System")).with_redaction(redact);
    let registry = CollectorRegistry::new();
    registry.collect_all(&mut current, redact);

    // Compare
    let diff = compare_snapshots(&base_snapshot, &current);

    info!(
        total_changes = diff.total_changes(),
        high_impact = diff.high_impact_count(),
        "Comparison complete"
    );

    // Update UI
    {
        let mut s = state.borrow_mut();
        s.current_diff = Some(diff.clone());

        if let Some(container) = &s.diff_container {
            // Clear container
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }

            // Add diff view
            let diff_view = create_diff_view(&diff);
            container.append(&diff_view);
        }

        if let Some(label) = &s.status_label {
            label.set_text(&format!(
                "{} changes found ({} high impact)",
                diff.total_changes(),
                diff.high_impact_count()
            ));
        }
    }
}

/// Deletes the selected snapshot.
fn delete_selected_snapshot(state: &Rc<RefCell<SnapshotPageState>>) {
    let snapshot_id = {
        let s = state.borrow();
        match s.selected_snapshot {
            Some(id) => id,
            None => return,
        }
    };

    {
        let s = state.borrow();
        if let Err(e) = s.store.delete(snapshot_id) {
            error!(error = %e, "Failed to delete snapshot");
            return;
        }
    }

    info!(id = %snapshot_id, "Deleted snapshot");

    // Clear selection and refresh
    {
        let mut s = state.borrow_mut();
        s.selected_snapshot = None;
        s.current_diff = None;

        if let Some(container) = &s.diff_container {
            while let Some(child) = container.first_child() {
                container.remove(&child);
            }
            let placeholder = create_diff_placeholder();
            container.append(&placeholder);
        }
    }

    refresh_snapshot_list(state);
}

/// Creates the diff results view.
fn create_diff_view(diff: &SnapshotDiff) -> ScrolledWindow {
    let scrolled = ScrolledWindow::new();
    scrolled.set_policy(PolicyType::Never, PolicyType::Automatic);
    scrolled.set_vexpand(true);

    let content = GtkBox::new(Orientation::Vertical, 16);
    content.set_margin_top(16);
    content.set_margin_bottom(16);

    if diff.total_changes() == 0 {
        // No changes
        let status = adw::StatusPage::new();
        status.set_icon_name(Some("document-properties-symbolic"));
        status.set_title(&tr("No Changes Detected"));
        status.set_description(Some(
            &tr("The current system state matches the snapshot."),
        ));
        content.append(&status);
    } else {
        // Summary header
        let summary = create_diff_summary(diff);
        content.append(&summary);

        // Category sections
        for category in DiffCategory::all() {
            let entries = diff.entries_by_category(*category);
            if !entries.is_empty() {
                let section = create_category_section(*category, entries);
                content.append(&section);
            }
        }
    }

    scrolled.set_child(Some(&content));
    scrolled
}

/// Creates a summary header for the diff.
fn create_diff_summary(diff: &SnapshotDiff) -> GtkBox {
    let summary = GtkBox::new(Orientation::Horizontal, 16);
    summary.add_css_class("card");
    summary.set_margin_start(8);
    summary.set_margin_end(8);

    // Total changes
    let total_box = create_stat_box(
        &diff.total_changes().to_string(),
        "Total Changes",
        None,
    );
    summary.append(&total_box);

    // High impact
    let high_box = create_stat_box(
        &diff.high_impact_count().to_string(),
        "High Impact",
        Some("error"),
    );
    summary.append(&high_box);

    // By category counts
    for category in DiffCategory::all() {
        let count = diff.entries_by_category(*category).len();
        if count > 0 {
            let cat_box = create_stat_box(
                &count.to_string(),
                category.label(),
                None,
            );
            summary.append(&cat_box);
        }
    }

    summary
}

/// Creates a stat box for the summary.
fn create_stat_box(value: &str, label: &str, css_class: Option<&str>) -> GtkBox {
    let stat_box = GtkBox::new(Orientation::Vertical, 4);
    stat_box.set_margin_top(16);
    stat_box.set_margin_bottom(16);
    stat_box.set_margin_start(16);
    stat_box.set_margin_end(16);

    let value_label = Label::new(Some(value));
    value_label.add_css_class("title-1");
    if let Some(class) = css_class {
        value_label.add_css_class(class);
    }
    stat_box.append(&value_label);

    let name_label = Label::new(Some(label));
    name_label.add_css_class("caption");
    name_label.add_css_class("dim-label");
    stat_box.append(&name_label);

    stat_box
}

/// Creates a section for a diff category.
fn create_category_section(category: DiffCategory, entries: &[DiffEntry]) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(category.label());
    group.set_description(Some(&format!("{} changes", entries.len())));

    for entry in entries {
        let row = create_diff_entry_row(entry);
        group.add(&row);
    }

    group
}

/// Creates a row for a diff entry.
fn create_diff_entry_row(entry: &DiffEntry) -> adw::ExpanderRow {
    let row = adw::ExpanderRow::new();
    row.set_title(&entry.name);

    // Subtitle with change type
    let subtitle = format!(
        "{} • {} impact",
        entry.change_type.label(),
        entry.impact.label()
    );
    row.set_subtitle(&subtitle);

    // Impact indicator
    row.add_css_class(entry.impact.css_class());
    row.add_css_class(entry.change_type.css_class());

    // Add icon based on change type
    let icon_name = match entry.change_type {
        ChangeType::Added => "list-add-symbolic",
        ChangeType::Removed => "list-remove-symbolic",
        ChangeType::Modified => "document-edit-symbolic",
    };
    row.add_prefix(&gtk4::Image::from_icon_name(icon_name));

    // Expanded content
    let content = GtkBox::new(Orientation::Vertical, 8);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(8);
    content.set_margin_bottom(8);

    // Explanation
    let explanation = Label::new(Some(&entry.explanation));
    explanation.set_wrap(true);
    explanation.set_halign(gtk4::Align::Start);
    explanation.set_xalign(0.0);
    content.append(&explanation);

    // Before/After values
    if entry.before.is_some() || entry.after.is_some() {
        let values_box = GtkBox::new(Orientation::Horizontal, 16);
        values_box.set_margin_top(8);

        if let Some(before) = &entry.before {
            let before_box = GtkBox::new(Orientation::Vertical, 4);
            let before_label = Label::new(Some(&tr("Before:")));
            before_label.add_css_class("caption");
            before_label.add_css_class("dim-label");
            before_label.set_halign(gtk4::Align::Start);
            before_box.append(&before_label);

            let before_value = Label::new(Some(before));
            before_value.add_css_class("monospace");
            before_value.set_halign(gtk4::Align::Start);
            before_box.append(&before_value);

            values_box.append(&before_box);
        }

        if let Some(after) = &entry.after {
            let after_box = GtkBox::new(Orientation::Vertical, 4);
            let after_label = Label::new(Some(&tr("After:")));
            after_label.add_css_class("caption");
            after_label.add_css_class("dim-label");
            after_label.set_halign(gtk4::Align::Start);
            after_box.append(&after_label);

            let after_value = Label::new(Some(after));
            after_value.add_css_class("monospace");
            after_value.set_halign(gtk4::Align::Start);
            after_box.append(&after_value);

            values_box.append(&after_box);
        }

        content.append(&values_box);
    }

    // Evidence (if any)
    if let Some(evidence) = &entry.evidence {
        let evidence_expander = gtk4::Expander::new(Some(&tr("Show Evidence")));
        let evidence_label = Label::new(Some(&evidence.content));
        evidence_label.add_css_class("monospace");
        evidence_label.set_wrap(true);
        evidence_label.set_halign(gtk4::Align::Start);
        evidence_label.set_xalign(0.0);
        evidence_expander.set_child(Some(&evidence_label));
        content.append(&evidence_expander);
    }

    row.add_row(&content);
    row
}
