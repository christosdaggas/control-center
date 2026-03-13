//! CSS styling and theme management.

use gtk4::{gdk, CssProvider};
use tracing::{debug, warn};

/// Application CSS styles.
const APP_CSS: &str = r#"
/* Severity colors - icon styling */
.severity-info {
    color: @success_color;
}

.severity-warning {
    color: @warning_color;
}

.severity-error {
    color: @error_color;
}

.severity-critical {
    color: @error_color;
    font-weight: bold;
}

/* Event row styling */
.event-row {
    padding: 12px;
    border-radius: 6px;
}

.event-row:hover {
    background-color: alpha(@accent_color, 0.1);
}

.event-summary {
    font-weight: 500;
}

.event-timestamp {
    font-size: 0.9em;
    opacity: 0.7;
}

.event-details {
    font-size: 0.9em;
    opacity: 0.8;
    margin-top: 4px;
}

/* Filter bar */
.filter-bar {
    padding: 8px 16px;
    background-color: alpha(@window_bg_color, 0.95);
}

.filter-preset-button {
    padding: 4px 12px;
    border-radius: 16px;
    font-size: 0.9em;
}

.filter-preset-button:checked {
    background-color: @accent_color;
    color: @accent_fg_color;
}

/* Correlation card */
.correlation-card {
    background-color: alpha(@accent_color, 0.05);
    border-radius: 12px;
    padding: 16px;
    margin: 8px 0;
}

.correlation-header {
    font-weight: 600;
    font-size: 1.1em;
}

.correlation-confidence {
    font-size: 0.85em;
    padding: 2px 8px;
    border-radius: 12px;
    background-color: alpha(@accent_color, 0.2);
}

.correlation-explanation {
    font-size: 0.9em;
    opacity: 0.8;
    margin-top: 8px;
}

/* Detail pane */
.detail-pane {
    padding: 16px;
}

.detail-header {
    font-weight: 600;
    font-size: 1.2em;
}

.evidence-box {
    background-color: alpha(@window_fg_color, 0.05);
    border-radius: 8px;
    padding: 12px;
    font-family: monospace;
    font-size: 0.9em;
}

/* Diagnostics page */
.diagnostics-row {
    padding: 8px 16px;
}

.diagnostics-label {
    font-weight: 500;
}

.diagnostics-value {
    opacity: 0.8;
}

.diagnostics-status-ok {
    color: @success_color;
}

.diagnostics-status-warning {
    color: @warning_color;
}

.diagnostics-status-error {
    color: @error_color;
}

/* Timeline view */
.timeline-view {
    padding: 16px;
}

.timeline-empty {
    padding: 48px;
    opacity: 0.6;
}

/* Severity badge */
.severity-badge {
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 0.8em;
    font-weight: 600;
    min-width: 60px;
    min-height: 20px;
}

.severity-badge.info {
    background-color: alpha(@success_color, 0.2);
    color: @success_color;
}

.severity-badge.warning {
    background-color: alpha(@warning_color, 0.2);
    color: @warning_color;
}

.severity-badge.error {
    background-color: alpha(@error_color, 0.2);
    color: @error_color;
}

.severity-badge.critical {
    background-color: @error_color;
    color: @accent_fg_color;
}

/* Navigation sidebar */
.navigation-sidebar {
    background-color: transparent;
}

.navigation-sidebar row {
    padding: 4px 4px;
    margin: 2px 8px;
    border-radius: 8px;
}

.navigation-sidebar row:selected {
    background-color: alpha(@accent_color, 0.2);
}

.navigation-sidebar row:hover:not(:selected) {
    background-color: alpha(@window_fg_color, 0.05);
}

/* Nav row box for sidebar items */
.nav-row-box {
    padding: 4px 0;
}

.nav-label {
    font-weight: 500;
}

/* Sidebar collapsed state */
.sidebar-collapsed {
    min-width: 50px;
}

.sidebar-collapsed .navigation-sidebar row {
    margin: 2px 4px;
}

.sidebar-collapsed .nav-row-box {
    padding: 4px 0;
}

/* Sidebar footer */
.sidebar-footer {
    background-color: alpha(@window_fg_color, 0.03);
    padding: 12px 16px;
    opacity: 0.7;
    font-size: 0.85em;
    border-top: 1px solid alpha(@window_fg_color, 0.1);
}

/* Sidebar container */
.sidebar-container {
    background-color: alpha(@window_fg_color, 0.08);
}

/* Update-available banner (sidebar footer) */
.update-banner {
    background-color: alpha(#e74c3c, 0.15);
    border: 1px solid alpha(#e74c3c, 0.3);
    border-radius: 6px;
    padding: 6px 10px;
    margin-bottom: 4px;
}

.update-banner .update-icon {
    color: #e74c3c;
}

.update-banner .update-link {
    font-size: 0.9em;
    font-weight: 600;
    color: #e74c3c;
    padding: 0;
    min-height: 0;
}

/* Theme selector popover */
.theme-selector {
    padding: 8px 16px;
}

.theme-button {
    min-width: 48px;
    min-height: 48px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    background: transparent;
}

.theme-button:checked {
    border-color: @accent_color;
}

.theme-button:hover:not(:checked) {
    background-color: alpha(@window_fg_color, 0.1);
}

.theme-selector {
    border-radius: 50%;
}

.theme-default {
    background: linear-gradient(135deg, #f0f0f0 50%, #303030 50%);
}

.theme-light {
    background-color: #f5f5f5;
    border: 1px solid alpha(#000, 0.1);
}

.theme-dark {
    background-color: #303030;
    border: 1px solid alpha(#fff, 0.1);
}

.theme-check {
    color: @accent_color;
    font-size: 14px;
}

/* Menu items */
.menu-item {
    padding: 8px 16px;
    min-height: 36px;
}

.menu-item:hover {
    background-color: alpha(@window_fg_color, 0.1);
}

/* Content header */
.content-header {
    padding: 0 16px;
}

/* Page container */
.page-container {
    background-color: @window_bg_color;
}

/* Placeholder page */
.placeholder-page {
    padding: 48px;
}

.placeholder-icon {
    opacity: 0.4;
    font-size: 64px;
}

.placeholder-title {
    font-size: 1.3em;
    font-weight: 600;
    margin-top: 16px;
}

.placeholder-subtitle {
    opacity: 0.6;
    margin-top: 8px;
}

/* Health page */
.health-page {
    background-color: @window_bg_color;
}

.health-page levelbar block.filled {
    background-color: @accent_color;
}

.health-page levelbar block.filled.high {
    background-color: @warning_color;
}

.health-page levelbar block.filled.full {
    background-color: @error_color;
}

/* ================================
   MODERN DASHBOARD STYLES
   ================================ */

/* Dashboard page */
.dashboard-page {
    background-color: @window_bg_color;
}

/* Header chips (hostname, OS) */
.chip {
    padding: 4px 10px;
    border-radius: 12px;
    background-color: alpha(@window_fg_color, 0.08);
}

.chip-label {
    font-size: 0.85em;
    font-weight: 500;
}

/* KPI Strip container */
.kpi-strip {
    margin-bottom: 8px;
}

/* KPI Tile */
.kpi-tile {
    background-color: alpha(@window_fg_color, 0.04);
    border-radius: 12px;
    padding: 10px 14px;
    min-height: 60px;
    min-width: 140px;
}

.kpi-tile:hover {
    background-color: alpha(@window_fg_color, 0.06);
}

.kpi-icon {
    opacity: 0.6;
}

.kpi-title {
    font-size: 0.85em;
    font-weight: 500;
    opacity: 0.7;
}

.kpi-value {
    font-size: 1.25em;
    font-weight: 700;
    margin-top: 2px;
}

.kpi-unit {
    font-size: 0.9em;
    opacity: 0.6;
}

.kpi-progress {
    margin-top: 6px;
    min-height: 4px;
}

.kpi-progress block.filled {
    background-color: @accent_color;
    border-radius: 2px;
}

/* Failed services badge in KPI */
.failed-badge {
    padding: 2px 8px;
    border-radius: 8px;
    background-color: alpha(@window_fg_color, 0.06);
    margin-left: 4px;
}

.failed-badge.has-failures {
    background-color: alpha(@error_color, 0.15);
}

.failed-count {
    font-size: 0.9em;
    font-weight: 600;
}

.failed-badge.has-failures .failed-count {
    color: @error_color;
}

.failed-text {
    font-size: 0.8em;
    opacity: 0.7;
}

.failed-badge.has-failures .failed-text {
    color: @error_color;
    opacity: 0.9;
}

/* Detail Cards */
.detail-card {
    background-color: alpha(@window_fg_color, 0.04);
    border-radius: 14px;
    padding: 18px 20px;
}

.card-title {
    font-size: 1.05em;
    font-weight: 600;
    margin-bottom: 12px;
    opacity: 0.9;
}

.card-row-label {
    font-weight: 500;
}

.card-value {
    font-weight: 600;
    font-size: 0.95em;
}

/* Info rows in cards */
.info-row {
    padding: 10px 0;
    border-bottom: 1px solid alpha(@window_fg_color, 0.06);
}

.info-row:last-child {
    border-bottom: none;
}

.info-label {
    font-size: 0.95em;
    opacity: 0.7;
}

.info-value {
    font-size: 0.95em;
    font-weight: 500;
}

/* Status rows */
.status-row {
    padding: 10px 0;
    border-bottom: 1px solid alpha(@window_fg_color, 0.06);
}

.status-row:last-child {
    border-bottom: none;
}

.status-dot {
    border-radius: 50%;
    margin-right: 4px;
    min-width: 8px;
    min-height: 8px;
}

.status-dot.status-running {
    background-color: @success_color;
}

.status-dot.status-failed {
    background-color: @error_color;
}

.status-dot.status-neutral {
    background-color: alpha(@window_fg_color, 0.3);
}

.status-label {
    font-size: 0.95em;
}

.status-value {
    font-size: 1.1em;
    font-weight: 600;
}

.status-value.status-running {
    color: @success_color;
}

.status-value.status-failed {
    color: @error_color;
}

.status-value.status-neutral {
    opacity: 0.7;
}

/* Disk card */
.disk-container {
    margin-top: 4px;
}

.disk-row {
    padding: 8px 0;
    border-bottom: 1px solid alpha(@window_fg_color, 0.06);
}

.disk-row:last-child {
    border-bottom: none;
}

.disk-mount {
    font-weight: 600;
    font-size: 0.95em;
    margin-bottom: 4px;
}

.disk-usage {
    font-size: 0.85em;
    opacity: 0.8;
}

.disk-progress {
    margin: 4px 0;
    min-height: 6px;
    border-radius: 3px;
}

.disk-progress block.filled {
    background-color: @accent_color;
    border-radius: 3px;
}

.disk-progress.disk-warning block.filled {
    background-color: @warning_color;
}

.disk-progress.disk-critical block.filled {
    background-color: @error_color;
}

.disk-subtitle {
    font-size: 0.85em;
    opacity: 0.5;
}

/* Empty state */
.empty-state {
    padding: 24px;
}

.empty-state-title {
    font-size: 1.05em;
    font-weight: 600;
    opacity: 0.8;
}

.empty-state-subtitle {
    font-size: 0.9em;
    opacity: 0.5;
    margin-top: 4px;
}

/* Services page */
.services-page {
    background-color: @window_bg_color;
}

.services-page .boxed-list {
    background-color: @card_bg_color;
    border-radius: 12px;
}

.services-page row.expander .title {
    font-weight: 500;
}

/* Service state colors */
.success {
    color: @success_color;
}

.warning {
    color: @warning_color;
}

.error {
    color: @error_color;
}

/* Linked button groups */
.linked > button {
    border-radius: 0;
}

.linked > button:first-child {
    border-top-left-radius: 6px;
    border-bottom-left-radius: 6px;
}

.linked > button:last-child {
    border-top-right-radius: 6px;
    border-bottom-right-radius: 6px;
}

/* Card style for logs */
.card {
    background-color: alpha(@window_fg_color, 0.05);
    border-radius: 8px;
    padding: 12px;
}

/* ================================
   BOX SHADOW / OUTLINE STYLES
   ================================ */

/* Shadow-like outline on all cards and detail cards */
.detail-card,
.kpi-tile,
.card,
.correlation-card {
    border: 1px solid alpha(@window_fg_color, 0.08);
    box-shadow: 0 1px 3px alpha(@window_fg_color, 0.04);
}

/* Stronger shadow in light mode via Adwaita's light palette */
window.background:not(.dark) .detail-card,
window.background:not(.dark) .kpi-tile,
window.background:not(.dark) .card,
window.background:not(.dark) .correlation-card {
    border: 1px solid alpha(@window_fg_color, 0.12);
    box-shadow: 0 1px 4px alpha(@window_fg_color, 0.08),
                0 0 1px alpha(@window_fg_color, 0.06);
}
"#;

/// Loads the application CSS.
pub fn load_css() {
    debug!("Loading application CSS");

    let provider = CssProvider::new();
    provider.load_from_string(APP_CSS);

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        debug!("CSS loaded successfully");
    } else {
        warn!("No display available, CSS not loaded");
    }
}
