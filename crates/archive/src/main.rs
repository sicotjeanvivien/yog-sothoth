//! `yog-archive` — the backup daemon.
//!
//! Every six hours it runs `pg_dump` against the database, streams the dump
//! into an S3-compatible bucket without writing it to disk, checks that what
//! it produced is a readable archive, and tells a dead man's switch
//! (Healthchecks.io) whether it succeeded. A missing signal is what raises the
//! alarm: it is the only kind of alert a stopped daemon can still trigger.
//!
//! It does not restore. Restoring is rare, done by hand, and needs someone to
//! choose the dump and the target; the proven sequence is in
//! `crates/persistence/README.md`, *Backup and restore*.

mod archiver;
mod bootstrap;
mod infra;
mod metrics;

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
        .inspect_err(|e| error!(error = %e, "failed to initialize the archiver"))?;
    info!("archiver initialized");

    // SIGTERM or Ctrl-C cancels the token; `Daemon::run` stops between two
    // dumps, or kills the one in progress. `shutdown_signal` listens for both:
    // under `docker compose stop` this process is PID 1, and SIGTERM is the
    // signal that arrives.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown_token.cancel();
    });

    daemon.run(token).await
}

/// Install the Prometheus exporter as the global `metrics` recorder, and
/// describe the archiver's metrics.
///
/// Exposes `http://0.0.0.0:9000/metrics`, like every daemon here. Must run
/// before any metric is emitted, in particular before `Daemon::new`, which
/// can signal a misconfigured bucket.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))?;
    metrics::register_descriptions();
    Ok(())
}
