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
//!
//! ## Graceful shutdown
//!
//! The process listens for SIGTERM (production) and SIGINT / Ctrl-C (dev). On
//! signal reception a [`CancellationToken`] is triggered: the daemon observes
//! it and waits for its three workers to finish their current tick before
//! returning — but for no longer than [`yog_bootstrap::SHUTDOWN_GRACE`], past
//! which the workers still running are named in the logs and destroyed with the
//! runtime.

mod bootstrap;
mod error;
mod providers;
mod source;
mod workers;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
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

    // ── Graceful shutdown ─────────────────────────────────────────────────────
    // Spawn a task that waits for SIGTERM / Ctrl-C, then cancels the shared
    // token. `Daemon::run` observes it, stops accepting new work, and waits for
    // its three workers before returning.
    //
    // ⚠️ **SIGTERM is the signal that matters, and it was not listened for.**
    // The daemon selected on `tokio::signal::ctrl_c()` alone, which is SIGINT.
    // Under `docker compose stop` — the way this process is actually stopped —
    // nothing happened at all: the compose service `exec`s the binary, so it is
    // PID 1, and the kernel does not deliver a signal's default action to
    // PID 1. The SIGTERM was simply ignored, and Docker waited its ten seconds
    // before SIGKILL. `shutdown_signal` covers both signals.
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
