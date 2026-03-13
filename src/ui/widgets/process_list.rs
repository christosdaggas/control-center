//! Process drilldown widget for the performance page.
//!
//! Shows the top-N resource-consuming processes with CPU and memory columns.

use crate::i18n::tr;
use crate::infrastructure::adapters::process::{format_bytes, ProcessAdapter, ProcessInfo};
use gtk4::prelude::*;
use gtk4::{self as gtk, glib};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use tracing::debug;

/// Sort mode for the process list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessSortMode {
    /// Sort by CPU usage (descending).
    #[default]
    Cpu,
    /// Sort by memory usage (descending).
    Memory,
}

/// Creates the process drilldown card widget.
///
/// Returns a `gtk4::Box` containing the top process list with auto-refresh.
pub fn create_process_drilldown() -> gtk::Box {
    debug!("Creating process drilldown widget");

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("detail-card");


    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_margin_bottom(12);

    let title = gtk::Label::new(Some(&tr("Top Processes")));
    title.add_css_class("card-title");
    title.set_halign(gtk::Align::Start);
    title.set_hexpand(true);
    header.append(&title);


    let sort_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    sort_box.add_css_class("linked");

    let cpu_btn = gtk::ToggleButton::with_label("CPU");
    cpu_btn.set_active(true);
    cpu_btn.set_tooltip_text(Some(&tr("Sort by CPU usage")));
    sort_box.append(&cpu_btn);

    let mem_btn = gtk::ToggleButton::with_label("Memory");
    mem_btn.set_group(Some(&cpu_btn));
    mem_btn.set_tooltip_text(Some(&tr("Sort by memory usage")));
    sort_box.append(&mem_btn);

    header.append(&sort_box);
    outer.append(&header);


    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::None);
    list_box.add_css_class("boxed-list");


    let col_header = create_column_header();
    list_box.append(&col_header);

    outer.append(&list_box);


    let sort_mode = Rc::new(RefCell::new(ProcessSortMode::Cpu));
    let adapter = Rc::new(RefCell::new(ProcessAdapter::new()));
    let list_ref = Rc::new(RefCell::new(list_box));


    {
        let sort_mode_c = sort_mode.clone();
        let adapter_c = adapter.clone();
        let list_c = list_ref.clone();
        cpu_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                *sort_mode_c.borrow_mut() = ProcessSortMode::Cpu;
                refresh_process_list(&adapter_c, &list_c, ProcessSortMode::Cpu);
            }
        });
    }
    {
        let sort_mode_c = sort_mode.clone();
        let adapter_c = adapter.clone();
        let list_c = list_ref.clone();
        mem_btn.connect_toggled(move |btn| {
            if btn.is_active() {
                *sort_mode_c.borrow_mut() = ProcessSortMode::Memory;
                refresh_process_list(&adapter_c, &list_c, ProcessSortMode::Memory);
            }
        });
    }


    refresh_process_list(&adapter, &list_ref, ProcessSortMode::Cpu);


    let adapter_timer = adapter.clone();
    let list_timer = list_ref.clone();
    let sort_timer = sort_mode.clone();
    glib::timeout_add_local(Duration::from_secs(3), move || {
        let mode = *sort_timer.borrow();
        refresh_process_list(&adapter_timer, &list_timer, mode);
        glib::ControlFlow::Continue
    });

    outer
}

/// Creates the column header row.
fn create_column_header() -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(6);
    hbox.set_margin_bottom(6);

    let name_label = gtk::Label::new(Some(&tr("Process")));
    name_label.set_halign(gtk::Align::Start);
    name_label.set_hexpand(true);
    name_label.add_css_class("dim-label");
    name_label.add_css_class("caption");
    hbox.append(&name_label);

    let pid_label = gtk::Label::new(Some("PID"));
    pid_label.set_width_chars(7);
    pid_label.set_halign(gtk::Align::End);
    pid_label.add_css_class("dim-label");
    pid_label.add_css_class("caption");
    hbox.append(&pid_label);

    let cpu_label = gtk::Label::new(Some(&tr("CPU")));
    cpu_label.set_width_chars(7);
    cpu_label.set_halign(gtk::Align::End);
    cpu_label.add_css_class("dim-label");
    cpu_label.add_css_class("caption");
    hbox.append(&cpu_label);

    let mem_label = gtk::Label::new(Some(&tr("Memory")));
    mem_label.set_width_chars(9);
    mem_label.set_halign(gtk::Align::End);
    mem_label.add_css_class("dim-label");
    mem_label.add_css_class("caption");
    hbox.append(&mem_label);

    row.set_child(Some(&hbox));
    row
}

/// Refreshes the process list.
fn refresh_process_list(
    adapter: &Rc<RefCell<ProcessAdapter>>,
    list_ref: &Rc<RefCell<gtk::ListBox>>,
    mode: ProcessSortMode,
) {
    let processes = {
        let mut a = adapter.borrow_mut();
        match mode {
            ProcessSortMode::Cpu => a.top_by_cpu(15),
            ProcessSortMode::Memory => a.top_by_memory(15),
        }
    };

    let list = list_ref.borrow();


    while let Some(row) = list.row_at_index(1) {
        list.remove(&row);
    }


    for proc_info in &processes {
        let row = create_process_row(proc_info);
        list.append(&row);
    }
}

/// Creates a row for a single process.
fn create_process_row(proc_info: &ProcessInfo) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);

    let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    hbox.set_margin_start(12);
    hbox.set_margin_end(12);
    hbox.set_margin_top(4);
    hbox.set_margin_bottom(4);


    let name_label = gtk::Label::new(Some(&proc_info.name));
    name_label.set_halign(gtk::Align::Start);
    name_label.set_hexpand(true);
    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    name_label.set_max_width_chars(30);
    name_label.set_tooltip_text(Some(&proc_info.cmdline));
    hbox.append(&name_label);


    let pid_label = gtk::Label::new(Some(&proc_info.pid.to_string()));
    pid_label.set_width_chars(7);
    pid_label.set_halign(gtk::Align::End);
    pid_label.add_css_class("dim-label");
    pid_label.add_css_class("monospace");
    hbox.append(&pid_label);


    let cpu_text = format!("{:.1}%", proc_info.cpu_percent);
    let cpu_label = gtk::Label::new(Some(&cpu_text));
    cpu_label.set_width_chars(7);
    cpu_label.set_halign(gtk::Align::End);
    cpu_label.add_css_class("monospace");
    if proc_info.cpu_percent > 80.0 {
        cpu_label.add_css_class("error");
    } else if proc_info.cpu_percent > 40.0 {
        cpu_label.add_css_class("warning");
    }
    hbox.append(&cpu_label);


    let mem_text = format_bytes(proc_info.rss_bytes);
    let mem_label = gtk::Label::new(Some(&mem_text));
    mem_label.set_width_chars(9);
    mem_label.set_halign(gtk::Align::End);
    mem_label.add_css_class("monospace");
    if proc_info.mem_percent > 20.0 {
        mem_label.add_css_class("error");
    } else if proc_info.mem_percent > 10.0 {
        mem_label.add_css_class("warning");
    }
    hbox.append(&mem_label);

    row.set_child(Some(&hbox));


    row.update_property(&[
        gtk::accessible::Property::Label(&format!(
            "{}, PID {}, CPU {:.1}%, Memory {}",
            proc_info.name,
            proc_info.pid,
            proc_info.cpu_percent,
            mem_text
        )),
    ]);

    row
}
