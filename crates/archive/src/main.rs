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
//!
//! Bootstrap follows the other daemons' shape, with one difference: the
//! configuration is read **before** the metrics exporter, because its listen
//! address is configurable — port 9000 is taken on a host already running the
//! other daemons' containers.

mod archiver;
mod bootstrap;
mod infra;
mod metrics;

use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    let config = bootstrap::Config::load()
        .inspect_err(|e| error!(error = %e, "failed to load configuration"))?;
    info!("configuration loaded");

    PrometheusBuilder::new()
        .with_http_listener(config.metrics_addr)
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
        .inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;
    metrics::register_descriptions();

    let daemon = bootstrap::Daemon::new(config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize the archiver"))?;
    info!("archiver initialized");

    daemon.run().await
}
