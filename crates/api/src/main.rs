mod bootstrap;
mod http;

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
    let app_state = bootstrap::AppState::build(config.clone())
        .await
        .inspect_err(|e| error!(error = ?e, "failed to build application state"))?;

    // ── HTTP server ───────────────────────────────────────────────────────────
    http::run(app_state, config.bind_addr)
        .await
        .inspect_err(|e| error!(error = ?e, "fatal error in HTTP server"))
}
