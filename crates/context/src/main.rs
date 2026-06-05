//! `yog-context` — token enrichment daemon.
//!
//! A standalone process: the 4th binary alongside `indexer`, `api`
//! and `web`. It enriches the raw on-chain data the indexer records:
//!
//!   - the metadata worker polls `pools` for new mints and fetches
//!     their identity (symbol, name, decimals, logo) from Helius DAS;
//!   - the price worker periodically fetches USD prices from Jupiter.
//!
//! Both persist through the `yog-persistence` repositories.
//!
//! Bootstrap follows the same shape as the other crates:
//! `init_rustls -> dotenv -> init_tracing -> Config -> AppState ->
//! run`.

mod bootstrap;
mod error;
mod providers;
mod source;
mod workers;

use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Process-level invariants ──────────────────────────────────────────────
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    let config = bootstrap::Config::load()?;
    info!("configuration loaded");

    let daemon = bootstrap::Daemon::new(&config)
        .await
        .inspect_err(|e| error!(error = %e, "failed to initialize daemon"))?;
    info!("daemon state initialized");

    daemon.run().await
}
