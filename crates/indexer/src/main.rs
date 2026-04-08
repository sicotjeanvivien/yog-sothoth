use crate::{config::Config, infra::Database};

pub(crate) mod application;
pub(crate) mod config;
pub(crate) mod domain;
pub(crate) mod infra;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = Config::load();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Connect to database
    let db = Database::connect(&config.database_url).await?;
    tracing::info!("connected to database");

    // Run migrations
    db.run_migrations().await?;
    tracing::info!("migrations applied");

    // TODO: initialize repositories
    // TODO: initialize services
    // TODO: start WebSocket RPC listener

    tracing::info!("indexer started");

    // Keep the process alive
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutting down");

    Ok(())
}
