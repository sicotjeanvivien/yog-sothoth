//! Shared daemon dependencies, assembled once at startup.
//!
//! Holds the repositories and the two HTTP source clients. The two
//! reqwest clients are deliberately distinct (and separate from the
//! indexer's RPC client): a burst of enrichment traffic must never
//! slow the indexer's hot ingestion path.

use std::sync::Arc;

use anyhow::Context;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_core::domain::{TokenMetadataRepository, TokenPriceRepository};
use yog_persistence::{Database, PgTokenMetadataRepository, PgTokenPriceRepository};

use crate::bootstrap::Config;
use crate::error::WorkerError;
use crate::providers::{HeliusDasClient, JupiterPriceClient};
use crate::source::{MetadataSource, PriceSource};
use crate::workers::{MetadataWorker, PriceWorker};

/// Dependencies shared by the daemon's workers.
#[derive(Clone)]
pub(crate) struct Daemon {
    /// Metadata persistence.
    token_metadata_repository: Arc<dyn TokenMetadataRepository>,
    /// Price persistence.
    token_price_repository: Arc<dyn TokenPriceRepository>,
    /// Helius DAS source client (metadata).
    metadata_source: Arc<dyn MetadataSource>,
    /// Jupiter price source client.
    price_source: Arc<dyn PriceSource>,
    /// Context METADATA poll secs
    poll_interval: std::time::Duration,
    /// context PRICE interval secs
    price_interval: std::time::Duration,
}

impl Daemon {
    /// Connect to the database, build the repositories and the source
    /// clients.
    pub(crate) async fn new(config: &Config) -> anyhow::Result<Self> {
        let database = init_db(config.database_url.expose())
            .await
            .context("database initialization failed")?;
        info!("database initialized");

        let poll_interval = config.metadata_poll_interval;
        let price_interval = config.price_interval;

        let db_pool = database.pool().clone();

        let token_metadata_repository: Arc<dyn TokenMetadataRepository> =
            Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));

        let token_price_repository: Arc<dyn TokenPriceRepository> =
            Arc::new(PgTokenPriceRepository::new(db_pool));

        // Two independent HTTP clients — one per external source.
        let metadata_source =
            Arc::new(HeliusDasClient::new(config.helius_url.expose().to_string()));
        let price_source = Arc::new(JupiterPriceClient::new(
            config.jupiter_url.expose().to_string(),
            config.jupiter_api_key.expose().to_string(),
        ));

        Ok(Self {
            token_metadata_repository,
            token_price_repository,
            metadata_source,
            price_source,
            poll_interval,
            price_interval,
        })
    }

    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();

        let metadata_task = spawn_metadata_worker(
            Arc::clone(&self.token_metadata_repository),
            self.metadata_source.clone(),
            self.poll_interval,
            shutdown.clone(),
        );
        let price_task = spawn_price_worker(
            Arc::clone(&self.token_metadata_repository),
            Arc::clone(&self.token_price_repository),
            self.price_source.clone(),
            self.price_interval,
            shutdown.clone(),
        );

        tokio::select! {
            result = metadata_task => {
                shutdown.cancel();
                handle_task_result(result, "metadata worker")?
            }
            result = price_task => {
                shutdown.cancel();
                handle_task_result(result, "price worker")?
            }
            _ = tokio::signal::ctrl_c() => {
                info!("ctrl-c received — stopping");
                shutdown.cancel();
            }
        }

        Ok(())
    }
}

/// Connect to the database.
///
/// The database URL is held in `Config::database_url` (a redacted secret),
/// so we never log it directly — `anyhow::Context` is sufficient to surface
/// the failure at startup without leaking credentials.
async fn init_db(database_url: &str) -> anyhow::Result<Database> {
    let db = Database::connect(database_url)
        .await
        .context("failed to connect to database")?;
    tracing::info!("connected to database");
    Ok(db)
}

/// Spawn the metadata worker task.
fn spawn_metadata_worker(
    repository: Arc<dyn TokenMetadataRepository>,
    metadata_source: Arc<dyn MetadataSource>,
    poll_interval: std::time::Duration,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = MetadataWorker::new(repository, metadata_source, poll_interval);
    tokio::spawn(async move { worker.run(shutdown).await })
}

/// Spawn the price worker task.
fn spawn_price_worker(
    metadata_repository: Arc<dyn TokenMetadataRepository>,
    price_repository: Arc<dyn TokenPriceRepository>,
    price_source: Arc<dyn PriceSource>,
    interval: std::time::Duration,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = PriceWorker::new(
        metadata_repository,
        price_repository,
        price_source,
        interval,
    );
    tokio::spawn(async move { worker.run(shutdown).await })
}

/// Normalise a finished task into a loggable `anyhow::Result`.
///
/// Distinguishes a clean stop, a worker error, and a task panic —
/// same three cases the indexer's `handle_task_result` covers.
fn handle_task_result(
    result: Result<Result<(), WorkerError>, tokio::task::JoinError>,
    task_name: &str,
) -> anyhow::Result<()> {
    match result {
        Ok(Ok(())) => {
            info!("{task_name} stopped");
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "{task_name} failed");
            Err(anyhow::Error::new(e))
        }
        Err(e) => {
            tracing::error!(error = %e, "{task_name} panicked");
            Err(anyhow::anyhow!("{task_name} panicked: {e}"))
        }
    }
}
