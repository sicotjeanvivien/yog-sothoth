mod application;
mod config;
mod domain;
mod infra;

use application::services::IndexerService;
use config::Config;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;

use crate::infra::{Database, RpcListener};

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

    // Initialize RPC client (HTTP) for transaction fetching
    let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.clone()));

    // Initialize indexer service
    let indexer_service = Arc::new(IndexerService::new(Arc::clone(&rpc_client)));

    // Initialize RPC listener (WebSocket)
    let mut listener =
        RpcListener::new(config.solana_rpc_ws.clone(), config.solana_rpc_http.clone());

    // Watch the test pool
    listener.watch("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j".to_string());

    info!("indexer started — watching test pool");

    // Start the WebSocket listener
    let service = Arc::clone(&indexer_service);
    listener
        .run(move |signature| {
            let service = Arc::clone(&service);
            async move {
                service.handle_signature(signature).await;
            }
        })
        .await;

    // Graceful shutdown on Ctrl+C
    tokio::signal::ctrl_c().await?;
    info!("shutting down");

    Ok(())
}
