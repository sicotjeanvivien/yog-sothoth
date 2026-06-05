//! `yog-context` — token enrichment daemon.
//!
//! A standalone process: the 4th binary alongside `indexer`, `api`
//! and `web`. It enriches the raw on-chain data the indexer records:
//!
//!   - the metadata worker polls `pools` for new mints and fetches
//!     their identity (symbol, name, decimals, logo) from Helius DAS;
//!   - the price worker periodically fetches USD prices from Jupiter.
//!
//! Both persist through the `yog-persistence` repositories.
//!
//! Bootstrap follows the same shape as the other crates:
//! `init_rustls -> dotenv -> init_tracing -> Config -> AppState ->
//! run`.

mod bootstrap;
mod error;
mod providers;
mod source;
mod workers;

use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Process-level invariants ──────────────────────────────────────────────
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    init_metrics().inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;

    let config = bootstrap::Config::load()?;
    info!("configuration loaded");

    let daemon = bootstrap::Daemon::new(&config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize daemon"))?;
    info!("daemon state initialized");

    daemon.run().await
}

/// Install the Prometheus exporter as the global `metrics` recorder.
///
/// Exposes `http://0.0.0.0:9000/metrics` in Prometheus text format.
/// Must be called before any metric is emitted, in particular before
/// `Daemon::new` which registers metric descriptions.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
}
