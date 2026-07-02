mod application;
mod bootstrap;
mod http;

#[cfg(test)]
mod testing;

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

    // ── Application state ─────────────────────────────────────────────────────
    let (app_state, signal_poller) = bootstrap::AppState::build(config.clone())
        .await
        .inspect_err(|e| error!(error = ?e, "failed to build application state"))?;

    // ── Signal stream poller ──────────────────────────────────────────────────
    // Feeds /api/signals/stream. Lives until the process dies — the api
    // has no graceful-shutdown path for it to hook into.
    tokio::spawn(signal_poller.run());

    // ── HTTP server ───────────────────────────────────────────────────────────
    http::run(app_state, config.bind_addr, config.cors_allowed_origins)
        .await
        .inspect_err(|e| error!(error = ?e, "fatal error in HTTP server"))
}
