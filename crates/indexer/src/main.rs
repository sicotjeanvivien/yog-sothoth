//! # yog-sothoth — Indexer entry point
//!
//! Entry point for the `indexer` binary.
//!
//! Single responsibility: initialize cross-cutting infrastructure
//! (logging, metrics, configuration) and delegate orchestration to [`Daemon`].
//!
//! ## Startup sequence
//!
//! ```text
//! main()
//!  ├─ init_tracing()      → structured logging (JSON prod / text dev)
//!  ├─ init_metrics()      → Prometheus exporter on :9000/metrics
//!  ├─ Config::load()      → explicit validation, no credentials in logs
//!  ├─ Daemon::new(config) → wires the dependency graph
//!  └─ Daemon::run()       → drives the indexer loop until shutdown signal
//! ```
//!
//! ## Error handling
//!
//! Fatal startup errors are logged via `tracing::error!` before the process
//! exits, so every crash produces a structured log entry visible in the
//! collector.
//!
//! ## Graceful shutdown
//!
//! The process listens for SIGTERM (production) and SIGINT / Ctrl-C (dev).
//! On signal reception, a [`CancellationToken`] is triggered: the daemon
//! observes it, stops accepting new work, and waits for in-flight tasks
//! (listener, dispatcher, indexer) to finish before returning.

// `application` - Contains the core business logic for the indexer.
mod application;

// `bootstrap` - Handles the initialization of the daemon and its dependencies.
mod bootstrap;

// `config` - Manages configuration loading and validation.
mod config;

// `error` - Defines IndexerError, the typed error enum for fatal startup failures.
mod error;

// `infra` - Provides infrastructure utilities (e.g., DB connections, RPC clients).
mod infra;

mod utils;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing_subscriber::EnvFilter;

use bootstrap::Daemon;
use config::Config;

// ── Logging ──────────────────────────────────────────────────────────────────

/// Initializes the tracing subscriber.
///
/// Format is selected from the `LOG_FORMAT` environment variable:
/// - `json` → machine-readable, suitable for log collectors (Loki, Datadog…)
/// - anything else → human-readable text, suitable for local development
///
/// Log level is controlled by `RUST_LOG` (defaults to `info`):
/// ```text
/// RUST_LOG=yog_indexer=debug,yog_core=debug,warn
/// ```
fn init_tracing() {
    let format = std::env::var("LOG_FORMAT").unwrap_or_default();

    // Respect RUST_LOG
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if format.eq_ignore_ascii_case("json") {
        tracing_subscriber::fmt()
            .json()
            .with_current_span(true)
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_target(true)
            .with_env_filter(filter)
            .init();
    }
}

// ── Metrics ──────────────────────────────────────────────────────────────────

/// Installs the Prometheus exporter as the global `metrics` recorder.
///
/// Exposes `http://0.0.0.0:9000/metrics` in Prometheus text format.
/// All `counter!()` / `histogram!()` calls elsewhere in the process are
/// silent no-ops until this runs — must be called before any metric is
/// emitted (notably before `Daemon::new`, which registers metric
/// descriptions).
///
/// # Errors
///
/// Returns an error if the listener address is already in use or if the
/// recorder has already been installed (both indicate a misconfiguration
/// and should stop the process).
fn init_metrics() -> anyhow::Result<()> {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .map_err(|e| anyhow::anyhow!("failed to install Prometheus exporter: {e}"))
}

// ── Shutdown signal ──────────────────────────────────────────────────────────

/// Resolves when SIGTERM **or** SIGINT (Ctrl-C) is received.
///
/// Using `tokio::select!` means whichever signal arrives first wins —
/// no double-handling, no extra state.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    // On non-Unix targets (e.g. Windows CI), only Ctrl-C is available.
    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c  => tracing::info!("received Ctrl-C — shutting down"),
        _ = sigterm => tracing::info!("received SIGTERM — shutting down"),
    }
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Entry point for the `indexer` binary.
///
/// Starts the multi-threaded Tokio runtime (default), suited for
/// I/O-bound workloads: Solana RPC WebSocket, TimescaleDB writes,
/// and WebSocket push to the frontend.
///
/// # Errors
///
/// Returns an error (and logs it) if:
/// - the Prometheus exporter cannot bind its listener
/// - the configuration is invalid or fails explicit validation
/// - TimescaleDB is unreachable at startup
/// - the Solana RPC connection is refused
/// - the indexing loop encounters an unrecoverable error
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Must be first: rustls 0.23 requires an explicit crypto provider.
    // Without this, any TLS connection (WS to Helius, HTTPS to the RPC)
    // panics on the first handshake.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    // ── Observability — must be first ─────────────────────────────────────────
    // Tracing and metrics are initialized before anything else so that
    // Config::load() errors and connection failures are captured as structured
    // log entries and counted in metrics.
    init_tracing();
    init_metrics().inspect_err(|e| error!(error = %e, "Failed to install metrics exporter"))?;

    // ── Configuration ─────────────────────────────────────────────────────────
    // `Config::load()` performs explicit validation of all required fields
    // (RPC URL, DB connection string, pool addresses…).
    //
    // SECURITY: Config's Display / Debug implementations MUST redact
    // credentials (DATABASE_URL password, API keys) before they reach the
    // log collector. See `utils::redact` for the masking logic.
    let config =
        Config::load().inspect_err(|e| error!(error = %e, "Failed to load configuration"))?;

    // ── Bootstrap ─────────────────────────────────────────────────────────────
    // `Daemon::new` wires the dependency graph: DB pool, RPC client,
    // watched-pool registry, metric descriptions. Validates live connections
    // before returning.
    let daemon = Daemon::new(config)
        .await
        .inspect_err(|e| error!(error = %e, "Failed to initialize daemon"))?;

    let token = CancellationToken::new();

    // ── Graceful shutdown ─────────────────────────────────────────────────────
    // Spawn a task that waits for SIGTERM / Ctrl-C, then cancels the token.
    // Daemon::run() observes the token and stops cleanly.
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_token.cancel();
    });

    // Drive the indexer loop — returns when the token is cancelled
    // or when an unrecoverable error occurs.
    daemon
        .run(token)
        .await
        .inspect_err(|e| error!(error = %e, "Fatal error in indexing loop"))
}
