use crate::bootstrap::{Server, config::Config};
use tracing::error;
use tracing_subscriber::EnvFilter;

mod bootstrap;
mod interface;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    // ── Configuration ─────────────────────────────────────────────────────────
    // `Config::load()` performs explicit validation of all required fields
    // (RPC URL, DB connection string, pool addresses…).
    //
    // SECURITY: Config's Display / Debug implementations MUST redact
    // credentials (DATABASE_URL password, API keys) before they reach the
    // log collector. See `utils::redact` for the masking logic.
    let config =
        Config::load().inspect_err(|e| error!(error = %e, "Failed to load configuration"))?;

    let server: Server = Server::init().await?;
    server.run().await;
    Ok(())
}
