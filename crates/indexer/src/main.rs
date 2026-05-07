//! # yog-sothoth — Indexer entry point
//!
//! Entry point for the `indexer` binary.
//!
//! Single responsibility: install process-level invariants (TLS crypto
//! provider, logging, metrics), load configuration, and delegate
//! orchestration to [`Daemon`].
//!
//! ## Startup sequence
//!
//! ```text
//! main()
//!  ├─ init_rustls()       → install rustls crypto provider (TLS prerequisite)
//!  ├─ dotenv()            → load .env file into the process environment
//!  ├─ init_tracing()      → structured logging (JSON prod / text dev)
//!  ├─ init_metrics()      → Prometheus exporter on :9000/metrics
//!  ├─ Config::load()      → explicit validation, no credentials in logs
//!  ├─ Daemon::new(config) → wires the dependency graph
//!  └─ Daemon::run(token)  → drives the indexer loop until shutdown signal
//! ```
//!
//! Order matters: `init_rustls` must run before any TLS connection is
//! attempted, `dotenv` before any environment variable is read, and
//! `init_tracing` before any log line is emitted.
//!
//! ## Error handling
//!
//! Every fatal startup error is logged via `tracing::error!` before the
//! process exits, so each crash leaves a structured entry visible in the
//! log collector.
//!
//! ## Graceful shutdown
//!
//! The process listens for SIGTERM (production) and SIGINT / Ctrl-C
//! (dev). On signal reception, a [`CancellationToken`] is triggered:
//! the daemon observes it, stops accepting new work, and waits for
//! in-flight tasks (listener, dispatcher, indexer) to finish before
//! returning.

// `application` — core business logic for the indexer.
mod application;

// `bootstrap` — daemon initialization, configuration loading, dependency wiring.
mod bootstrap;

// `error` — typed error enums for fatal startup and runtime failures.
mod error;

// `infra` — infrastructure utilities (RPC client, dispatcher, listener…).
mod infra;

mod utils;

use metrics_exporter_prometheus::PrometheusBuilder;
use tokio_util::sync::CancellationToken;
use tracing::error;

use bootstrap::{Config, Daemon};

// ── Metrics ──────────────────────────────────────────────────────────────────

/// Install the Prometheus exporter as the global `metrics` recorder.
///
/// Exposes `http://0.0.0.0:9000/metrics` in Prometheus text format.
/// All `counter!()` / `histogram!()` calls elsewhere in the process are
/// silent no-ops until this runs — must be called before any metric is
/// emitted (notably before `Daemon::new`, which registers metric
/// descriptions).
///
/// Local to the indexer: the api will expose its own metrics on its
/// HTTP server when needed, so this isn't worth promoting to
/// `yog-bootstrap`.
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

/// Resolve when SIGTERM **or** SIGINT (Ctrl-C) is received.
///
/// `tokio::select!` makes whichever signal arrives first win — no
/// double-handling, no extra state.
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
/// Starts the multi-threaded Tokio runtime (default), suited for the
/// I/O-bound workload: Solana RPC WebSocket, TimescaleDB writes, RPC
/// HTTP fetches.
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
    // ── Process-level invariants ──────────────────────────────────────────────
    // These three calls install global state that the rest of the process
    // depends on. Order matters and is enforced by the runtime, not the
    // type system — be careful when reordering.

    // rustls 0.23 requires an explicit crypto provider; without this,
    // the first TLS handshake panics. Must run before any TLS connection.
    yog_bootstrap::init_rustls();

    // Load `.env` into `std::env`. Silently ignored if the file is missing
    // — `Config::load()` will raise an explicit error per missing variable.
    dotenvy::dotenv().ok();

    // Install the global tracing subscriber. Reads `LOG_FORMAT` (json|text)
    // and `RUST_LOG` from the environment.
    yog_bootstrap::init_tracing();

    // Install the Prometheus exporter. Must run before any metric is emitted.
    init_metrics().inspect_err(|e| error!(error = %e, "failed to install metrics exporter"))?;

    // ── Configuration ─────────────────────────────────────────────────────────
    // `Config::load()` performs explicit validation of all required
    // variables (RPC URLs, DB connection string, retry budget, …) and
    // wraps secrets in `SecretUrl` so they never reach the log collector
    // through `Display` / `Debug`.
    let config =
        Config::load().inspect_err(|e| error!(error = %e, "failed to load configuration"))?;

    // ── Bootstrap ─────────────────────────────────────────────────────────────
    // `Daemon::new` wires the dependency graph: DB pool, RPC client,
    // watched-pool registry, metric descriptions. Validates live
    // connections before returning, so any failure here means the
    // process cannot run.
    let daemon = Daemon::new(config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize daemon"))?;

    // ── Graceful shutdown ─────────────────────────────────────────────────────
    // Spawn a task that waits for SIGTERM / Ctrl-C, then cancels the
    // shared token. `Daemon::run` observes the token and stops cleanly.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_token.cancel();
    });

    // Drive the indexer loop. Returns when the token is cancelled or
    // when an unrecoverable error occurs in one of the spawned tasks.
    daemon
        .run(token)
        .await
        .inspect_err(|e| error!(error = %e, "fatal error in indexing loop"))
}
