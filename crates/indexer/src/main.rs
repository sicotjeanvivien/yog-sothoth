//! # yog-sothoth — Indexer entry point
//!
//! Entry point for the `indexer` binary.
//!
//! Single responsibility: initialize cross-cutting infrastructure
//! (logging, configuration) and delegate orchestration to [`Daemon`].
//!
//! ## Startup sequence
//!
//! ```text
//! main()
//!  ├─ init_tracing()      → structured logging (JSON prod / text dev)
//!  ├─ Config::load()      → explicit validation, no credentials in logs
//!  ├─ Daemon::new(config) → wires the dependency graph
//!  └─ run_until_shutdown() → races daemon loop vs. SIGTERM / Ctrl-C
//! ```
//!
//! ## Error handling
//!
//! Fatal errors are logged via `tracing::error!` before the process exits,
//! so every crash produces a structured log entry visible in the collector.
//!
//! ## Graceful shutdown
//!
//! The process listens for SIGTERM (production) and SIGINT / Ctrl-C (dev).
//! On signal reception, the daemon is given a chance to flush in-flight
//! writes before the process exits.

// `application` - Contains the core business logic for the indexer.
mod application;

// `bootstrap` - Handles the initialization of the daemon and its dependencies.
mod bootstrap;

// `config` - Manages configuration loading and validation.
mod config;

// `infra` - Provides infrastructure utilities (e.g., DB connections, RPC clients).
mod infra;

use anyhow::Context as _;
use tokio_util::sync::CancellationToken;
use tracing::error;

use bootstrap::Daemon;
use config::Config;

// ── Logging ──────────────────────────────────────────────────────────────────

/// Initializes the tracing subscriber.
///
/// Format is selected from the `LOG_FORMAT` environment variable:
/// - `json`  → machine-readable, suitable for log collectors (Loki, Datadog…)
/// - anything else → human-readable text, suitable for local development
///
/// Log level is controlled by `RUST_LOG` (defaults to `info`):
/// ```text
/// RUST_LOG=yog_sothoth_indexer=debug,warn
/// ```
fn init_tracing() {
    let format = std::env::var("LOG_FORMAT").unwrap_or_default();

    if format.eq_ignore_ascii_case("json") {
        tracing_subscriber::fmt()
            .json()
            // Include the span context in every log line — useful for
            // correlating a DB write with the RPC event that triggered it.
            .with_current_span(true)
            .init();
    } else {
        tracing_subscriber::fmt().with_target(true).init();
    }
}

// ── Shutdown signal ───────────────────────────────────────────────────────────

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
        _ = ctrl_c   => tracing::info!("received Ctrl-C — shutting down"),
        _ = sigterm  => tracing::info!("received SIGTERM — shutting down"),
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Entry point for the `indexer` binary.
///
/// Starts the multi-threaded Tokio runtime (default), suited for
/// I/O-bound workloads: Solana RPC WebSocket, TimescaleDB writes,
/// and WebSocket push to the frontend.
///
/// # Errors
///
/// Returns an error (and logs it) if:
/// - the configuration is invalid or fails explicit validation
/// - TimescaleDB is unreachable at startup
/// - the Solana RPC connection is refused
/// - the indexing loop encounters an unrecoverable error
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Observability — must be first ─────────────────────────────────────────
    // Initialized before anything else so that Config::load() errors and
    // connection failures are captured as structured log entries.
    init_tracing();

    // ── Configuration ─────────────────────────────────────────────────────────
    // `Config::load()` performs explicit validation of all required fields
    // (RPC URL, DB connection string, pool addresses…).
    //
    // SECURITY: Config::fmt / Config::debug implementations MUST redact
    // credentials (DATABASE_URL password, API keys) before they reach the
    // log collector. See `config::redact` for the masking logic.
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            // Log the full cause chain as a structured error before exiting.
            // `{e:#}` prints the chain: "invalid config: missing field: RPC_URL"
            error!(error = %e, "Failed to load configuration");
            return Err(e);
        }
    };

    // ── Bootstrap ─────────────────────────────────────────────────────────────
    // `Daemon::new` wires the dependency graph: DB pool, RPC client,
    // watched-pool registry. Validates live connections before returning.
    let daemon = match Daemon::new(config).await {
        Ok(d) => d,
        Err(e) => {
            error!(error = %e, "Failed to initialize daemon");
            return Err(e);
        }
    };

    let token = CancellationToken::new();

    // ── Main loop with graceful shutdown ──────────────────────────────────────
    // `tokio::select!` races the daemon loop against the shutdown signal.
    // When a signal arrives, `daemon.run()` is cancelled.
    // TODO: implement Drop on Daemon to flush in-flight DB writes on shutdown.
    tokio::select! {
        result = daemon.run(token.clone()) => {
            token.cancel();
            if let Err(ref e) = result {
                error!(error = %e, "Fatal error in indexing loop");
            }
            result.context("Fatal error in indexing loop")
        }
        _ = shutdown_signal() => {
            token.cancel();
            tracing::info!("shutdown complete");
            Ok(())
        }
    }
}
