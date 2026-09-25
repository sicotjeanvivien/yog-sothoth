mod application;
mod bootstrap;
mod http;

#[cfg(test)]
mod testing;

use tokio_util::sync::CancellationToken;
use tracing::error;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Process-level invariants ──────────────────────────────────────────────
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    // ── Configuration ─────────────────────────────────────────────────────────
    let config = bootstrap::Config::load()
        .inspect_err(|e| error!(error = ?e, "failed to load configuration"))?;

    // ── Graceful shutdown ─────────────────────────────────────────────────────
    // SIGTERM or Ctrl-C cancels the token; `serve` stops the server and the
    // poller, and the SSE streams end with it. `shutdown_signal` listens for
    // both: under `docker compose stop` this process is PID 1, and a PID 1
    // with no SIGTERM handler does not die of the signal — it ignores it, and
    // Docker kills it ten seconds later.
    let token = CancellationToken::new();
    let shutdown_token = token.clone();
    tokio::spawn(async move {
        yog_bootstrap::shutdown_signal().await;
        shutdown_token.cancel();
    });

    // ── Application state ─────────────────────────────────────────────────────
    let (app_state, signal_poller) = bootstrap::AppState::build(config.clone(), token.clone())
        .await
        .inspect_err(|e| error!(error = ?e, "failed to build application state"))?;

    // ── HTTP server and signal stream poller ──────────────────────────────────
    let listener = http::bind(config.bind_addr)
        .await
        .inspect_err(|e| error!(error = ?e, "failed to bind the API listener"))?;
    let router = http::build_router(app_state, config.cors_allowed_origins);
    bootstrap::serve(listener, router, signal_poller, token)
        .await
        .inspect_err(|e| error!(error = ?e, "fatal error in the API"))
}
