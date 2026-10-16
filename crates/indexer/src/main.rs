//! Entry point for the `indexer` binary: installs the process-level invariants (TLS provider,
//! logging, metrics), loads the configuration, and hands over to [`Daemon`].
//! ⚠️ Returning from `main` drops the runtime, cancelling every task wherever it stands: the
//! daemon's wait, bounded by [`yog_bootstrap::SHUTDOWN_GRACE`], is what keeps a stop orderly.

mod application;
mod bootstrap;
mod error;
mod infra;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
use tracing::error;

use bootstrap::{Config, Daemon};

/// Install the Prometheus exporter on `:9000/metrics` as the global `metrics` recorder.
/// ⚠️ Every `counter!()` is a silent no-op until then, so it runs before `Daemon::new`.
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
}

/// Entry point for the `indexer` binary; every fatal error is logged before the process exits.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ⚠️ Order matters, enforced by the runtime, not the type system: TLS provider before any
    // connection, `.env` before any variable is read, tracing before any log line.

    // rustls 0.23 needs an explicit provider, or the first TLS handshake panics.
    yog_bootstrap::init_rustls();

    // A missing `.env` is ignored: `Config::load` names each missing variable.
    dotenvy::dotenv().ok();

    // Reads `LOG_FORMAT` and `RUST_LOG`.
    yog_bootstrap::init_tracing();

    init_metrics().inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;

    let config =
        Config::load().inspect_err(|e| error!(error = %e, "failed to load configuration"))?;

    let daemon = Daemon::new(config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize daemon"))?;

    // SIGTERM or Ctrl-C cancels the shared token, which `Daemon::run` observes.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown_token.cancel();
    });

    daemon
        .run(token)
        .await
        .inspect_err(|e| error!(error = %e, "fatal error in indexing loop"))
}
