//! `yog-archive`, the backup daemon: streams `pg_dump` into an S3-compatible
//! bucket without touching disk, checks it reads as an archive, and tells a dead man's switch.
//! It does not restore: that is by hand, per `crates/persistence/README.md`, *Backup and restore*.

mod application;
mod bootstrap;
mod infra;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    init_metrics().inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;

    let config = bootstrap::Config::load()
        .inspect_err(|e| error!(error = %e, "failed to load configuration"))?;
    info!("configuration loaded");

    let daemon = bootstrap::Daemon::connect(config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize the archiver"))?;
    info!("archiver initialized");

    // SIGTERM or Ctrl-C cancels the token: as PID 1 under compose, SIGTERM is what arrives.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown_token.cancel();
    });

    daemon.run(token).await;
    Ok(())
}

/// Install the Prometheus exporter on `:9000/metrics` and describe the archiver's metrics.
/// ⚠️ Before any metric is emitted, so before `Daemon::connect`, which can signal a misconfigured bucket.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))?;
    application::metrics::register_descriptions();
    Ok(())
}
