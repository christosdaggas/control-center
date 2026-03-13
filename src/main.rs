//! Control Center - Application Entry Point
//!
//! Initializes logging, loads configuration, and starts the GTK4 application.
//! Supports `--create-snapshot <name>` for headless snapshot creation (used by auto-snapshot timer).

use control_center::ui::ControlCenterApp;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() -> anyhow::Result<()> {

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(true).with_thread_ids(true))
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("control_center=info,warn")),
        )
        .init();

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting Control Center"
    );

    control_center::i18n::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 3 && args[1] == "--create-snapshot" {
        let name = &args[2];
        info!(name = %name, "Creating headless snapshot");
        return create_snapshot_cli(name);
    }

    if args.len() >= 2 && args[1] == "--run-retention" {
        info!("Running data retention cleanup");
        return run_retention_cli();
    }


    let exit_code = ControlCenterApp::run();

    std::process::exit(exit_code);
}

/// Creates a snapshot from the CLI without launching the GUI.
fn create_snapshot_cli(name: &str) -> anyhow::Result<()> {
    use control_center::domain::snapshot::Snapshot;
    use control_center::infrastructure::adapters::snapshot::CollectorRegistry;
    use control_center::infrastructure::storage::SnapshotStore;

    let mut snapshot = Snapshot::new(name);
    let registry = CollectorRegistry::new();
    registry.collect_all(&mut snapshot, false);
    snapshot = snapshot.with_description("Auto-created by scheduled timer");

    let store = SnapshotStore::new()?;
    store.save(&snapshot)?;

    info!(
        id = %snapshot.id,
        name = %snapshot.name,
        "Snapshot created successfully"
    );
    Ok(())
}

/// Runs data retention cleanup from the CLI.
fn run_retention_cli() -> anyhow::Result<()> {
    use control_center::config::Config;
    use control_center::infrastructure::storage::retention::run_retention;

    let config = Config::load().unwrap_or_default();
    let result = run_retention(config.data_retention_days, 0);
    info!("{}", result);

    if result.errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("Retention cleanup had errors: {:?}", result.errors)
    }
}
