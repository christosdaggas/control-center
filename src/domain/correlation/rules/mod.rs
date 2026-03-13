//! Built-in correlation rules.
//!
//! Each rule is a self-contained module implementing the Rule trait.

mod config_service_reload;
mod disk_write_failure;
mod network_service_timeout;
mod oom_service_restart;
mod package_service_restart;
mod permission_denial;
mod service_cascade;

pub use config_service_reload::ConfigServiceReloadRule;
pub use disk_write_failure::DiskWriteFailureRule;
pub use network_service_timeout::NetworkServiceTimeoutRule;
pub use oom_service_restart::OomServiceRestartRule;
pub use package_service_restart::PackageServiceRestartRule;
pub use permission_denial::PermissionDenialImpactRule;
pub use service_cascade::ServiceCascadeFailureRule;
