//! `yog-context`, the token enrichment daemon: the metadata worker (Helius DAS), the price
//! worker (Jupiter) and the pool-account worker (on-chain accounts).
//! ⚠️ On a stop, the daemon waits for the three workers, bounded by
//! [`yog_bootstrap::SHUTDOWN_GRACE`]; returning from `main` destroys what still runs.

mod application;
mod bootstrap;
mod error;
mod infra;
#[cfg(test)]
mod testing;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    init_metrics().inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;

    let config = bootstrap::Config::load()?;
    info!("configuration loaded");

    let daemon = bootstrap::Daemon::connect(&config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize daemon"))?;
    info!("daemon state initialized");

    // ⚠️ SIGTERM, not only Ctrl-C: under compose the binary is PID 1, which the kernel spares a
    // signal's default action, so an unheard SIGTERM waits Docker's ten seconds for SIGKILL.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown_token.cancel();
    });

    daemon
        .run(token)
        .await
        .inspect_err(|e| error!(error = %e, "fatal error in the context daemon"))
}

/// Install the Prometheus exporter on `:9000/metrics` as the global `metrics` recorder.
/// ⚠️ Before any metric is emitted, so before `Daemon::connect` registers the descriptions.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
}
