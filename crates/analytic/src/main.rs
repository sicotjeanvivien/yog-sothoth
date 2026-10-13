//! `yog-analytic` — values the swaps once, per pool and hour, into the
//! analytic tables, derives the protocol aggregate and TVL from them, and
//! freezes each hour once closed. Writes under the `yog_analytic` role.

mod application;
mod bootstrap;

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

    let daemon = bootstrap::Daemon::new(config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize the analytic writer"))?;
    info!("analytic writer initialized");

    let token = CancellationToken::new();
    let shutdown = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown.cancel();
    });

    daemon.run(token).await;
    Ok(())
}

fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))?;
    application::metrics::register_descriptions();
    Ok(())
}
