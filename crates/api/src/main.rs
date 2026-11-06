mod application;
mod bootstrap;
mod http;

#[cfg(test)]
mod testing;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
use tracing::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    init_metrics().inspect_err(|e| error!(error = ?e, "failed to install the metrics exporter"))?;

    let config = bootstrap::Config::load()
        .inspect_err(|e| error!(error = ?e, "failed to load configuration"))?;

    // ⚠️ Under `docker compose stop` this process is PID 1: without a SIGTERM handler it
    // ignores the signal and Docker kills it ten seconds later.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown_token.cancel();
    });

    let (app_state, live_pollers, key_ring) =
        bootstrap::build_app_state(config.clone(), token.clone())
            .await
            .inspect_err(|e| error!(error = ?e, "failed to build application state"))?;

    let listener = http::bind(config.bind_addr)
        .await
        .inspect_err(|e| error!(error = ?e, "failed to bind the API listener"))?;
    let router = http::build_router(app_state, config.cors_allowed_origins);
    bootstrap::serve(listener, router, live_pollers, key_ring, token)
        .await
        .inspect_err(|e| error!(error = ?e, "fatal error in the API"))
}

/// Prometheus on `:9000`, as the daemons: `observe.sh` reads it through the compose network.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))?;
    application::metrics::ApiMetrics::register_descriptions();
    Ok(())
}
