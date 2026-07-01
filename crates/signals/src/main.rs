//! `signal-engine` — the Signal Engine daemon.
//!
//! The 5th backend binary alongside `indexer`, `api`, `context` and the
//! `web` BFF. It runs the detectors from the `yog-signals` library on their
//! own cadences and persists the signals they emit to the `signals` table,
//! under the least-privilege `yog_signals` role.
//!
//! Bootstrap follows the same shape as the other crates:
//! `init_rustls -> dotenv -> init_tracing -> metrics -> Config -> Daemon ->
//! run`.

mod bootstrap;

use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    init_metrics().inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;

    let config = bootstrap::Config::load()?;
    info!("configuration loaded");

    let daemon = bootstrap::Daemon::new(&config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize signal engine"))?;
    info!("signal engine initialized");

    daemon.run().await
}

/// Install the Prometheus exporter as the global `metrics` recorder.
///
/// Exposes `http://0.0.0.0:9000/metrics` in Prometheus text format (mapped
/// to a distinct host port by docker-compose). Must be called before any
/// metric is emitted, in particular before `Daemon::new`.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
}
