mod bootstrap;
mod interface;

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

    // ── Application app_state ─────────────────────────────────────────────────
    // Builds the dependency graph: DB pool, repositories, services. Validates
    // live connections (DB) before returning, so any failure here means the
    // process cannot run.
    let app_state = bootstrap::AppState::build(config.clone())
        .await
        .inspect_err(|e| error!(error = ?e, "failed to build application app_state"))?;

    // ── HTTP server ───────────────────────────────────────────────────────────
    // Wires the router from the app_state, binds the TCP listener, then runs
    // the accept loop until Ctrl-C.
    let server = bootstrap::Server::init(app_state, config)
        .await
        .inspect_err(|e| error!(error = ?e, "failed to initialize server"))?;

    server.run().await;

    Ok(())
}
