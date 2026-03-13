//! UI Pages.

pub mod diagnostics_page;
pub mod health_page;
pub mod help_page;
pub mod performance_page;
pub mod security_page;
pub mod services_page;
pub mod settings_page;
pub mod snapshot_page;
pub mod timeline_page;

pub use diagnostics_page::DiagnosticsPage;
pub use health_page::SystemHealthPage;
pub use help_page::create_help_page;
pub use performance_page::PerformancePage;
pub use security_page::SecurityPage;
pub use services_page::ServicesPage;
pub use settings_page::create_settings_page;
pub use snapshot_page::create_snapshot_page;
pub use timeline_page::TimelinePage;
