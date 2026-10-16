//! `yog-signals`, the Signal Engine daemon: runs its detectors on their own cadences and persists
//! what they emit to `signals`, under the least-privilege `yog_signals` role.

mod application;
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

/// Install the Prometheus exporter on `:9000/metrics` as the global `metrics` recorder.
/// ⚠️ Before any metric is emitted, so before `Daemon::new`.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
}
