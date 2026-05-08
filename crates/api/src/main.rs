mod axum_app;
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

    // ── Application state ─────────────────────────────────────────────────────
    let app_state = bootstrap::AppState::build(config.clone())
        .await
        .inspect_err(|e| error!(error = ?e, "failed to build application state"))?;

    // ── Servers ───────────────────────────────────────────────────────────────
    // Custom HTTP stack on `config.bind_addr` (production endpoint, unchanged)
    // and the new axum stack on AXUM_BIND_TRANSITIONAL in parallel during
    // the migration. Both consume the same `AppState`.
    //
    // Commit 3 will retire the custom stack and promote axum to
    // `config.bind_addr`.
    let custom_server = bootstrap::Server::init(app_state.clone(), config)
        .await
        .inspect_err(|e| error!(error = ?e, "failed to initialize custom server"))?;

    let axum_state = app_state.clone();
    let axum_handle = tokio::spawn(async move {
        if let Err(e) = axum_app::run(axum_state).await {
            error!(error = ?e, "axum server stopped unexpectedly");
        }
    });

    custom_server.run().await;

    // If the custom server returns (Ctrl-C), stop the axum task too.
    axum_handle.abort();

    Ok(())
}
