//! Shared daemon dependencies, assembled once at startup: the repositories,
//! the three source clients, each with its own HTTP client, and the heartbeat.

use std::sync::Arc;

use anyhow::Context;
use tokio::task::{JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;
use yog_bootstrap::{Heartbeat, Stop, handle_task_result};
use yog_core::domain::{
    PoolAccountResolver, PoolRepository, TokenMetadataRepository, TokenPriceRepository,
};
use yog_persistence::{
    PgMeteoraDammV2PoolPropertiesRepository, PgMeteoraDlmmPoolPropertiesRepository,
    PgPoolRepository, PgTokenMetadataRepository, PgTokenPriceRepository,
};

use crate::application::heartbeat::HeartbeatReporter;
use crate::application::heartbeat::source_health::SourceHealth;
use crate::application::source::{MetadataSource, PoolAccountSource, PriceSource};
use crate::application::workers::{MetadataWorker, PoolAccountWorker, PriceWorker};
use crate::bootstrap::Config;
use crate::error::WorkerError;
use crate::infra::{HeliusDasClient, JupiterPriceClient, SolanaAccountClient};

mod init;
mod log;

use init::{init_db, init_heartbeat, register_metric_descriptions};

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
    /// Per-protocol satellite persistence for account-derived properties.
    pool_account_resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    /// The neutral `pools` registry — the other half of each decoded account.
    pool_repository: Arc<dyn PoolRepository>,
    /// cp-amm pool account source.
    pool_account_source: Arc<dyn PoolAccountSource>,
    /// The metadata and pool-account workers' cadence.
    poll_interval: std::time::Duration,
    /// The price worker's cadence.
    price_interval: std::time::Duration,
    /// What the sources answered since the last ping, fed by the workers and the price client.
    health: Arc<SourceHealth>,
    /// `None` in development: the sources are counted, nothing is signalled.
    heartbeat: Option<Arc<dyn Heartbeat>>,
}

impl Daemon {
    /// Connect to the database, build the repositories and the source clients.
    pub(crate) async fn connect(config: &Config) -> anyhow::Result<Self> {
        let database = init_db(&config.database_url)
            .await
            .context("database initialization failed")?;
        log::database_initialized();

        let poll_interval = config.metadata_poll_interval;
        let price_interval = config.price_interval;

        let db_pool = database.pool().clone();

        let token_metadata_repository: Arc<dyn TokenMetadataRepository> =
            Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));

        let token_price_repository: Arc<dyn TokenPriceRepository> =
            Arc::new(PgTokenPriceRepository::new(db_pool.clone()));

        // One resolver per protocol, each with its own queue and tables; a new protocol pushes its own.
        let pool_account_resolvers: Vec<Arc<dyn PoolAccountResolver>> = vec![
            Arc::new(PgMeteoraDammV2PoolPropertiesRepository::new(
                db_pool.clone(),
            )),
            Arc::new(PgMeteoraDlmmPoolPropertiesRepository::new(db_pool.clone())),
        ];
        // Same worker, same account read, but through the repository that owns
        // `pools`: no satellite writes the cross-protocol registry.
        let pool_repository: Arc<dyn PoolRepository> = Arc::new(PgPoolRepository::new(db_pool));

        // Each client takes the wrapped secret: `.expose()` happens at the request builder.
        let metadata_source = Arc::new(HeliusDasClient::new(config.token_metadata.url()));
        let health = Arc::new(SourceHealth::default());
        let price_source = Arc::new(JupiterPriceClient::new(
            config.jupiter_url.clone(),
            config.jupiter_api_key.clone(),
            config.jupiter_rate_limit,
            Arc::clone(&health),
        ));
        log::jupiter_spacing(config.jupiter_rate_limit, price_source.request_spacing());
        // `getMultipleAccounts` has its own endpoint, maybe another provider than DAS: both are
        // logged side by side at start-up.
        let pool_account_source: Arc<dyn PoolAccountSource> =
            Arc::new(SolanaAccountClient::new(config.pool_account.url()));
        log::endpoints_initialized(&config.token_metadata, &config.pool_account);

        let heartbeat = config
            .heartbeat_url
            .clone()
            .map(init_heartbeat)
            .transpose()?;
        log::heartbeat_configured(heartbeat.is_some());

        register_metric_descriptions();

        Ok(Self {
            token_metadata_repository,
            token_price_repository,
            metadata_source,
            price_source,
            pool_account_resolvers,
            pool_repository,
            pool_account_source,
            poll_interval,
            price_interval,
            health,
            heartbeat,
        })
    }

    /// Run the workers and, when configured, the reporter until one ends or the stop is asked for, then wait for the others.
    /// ⚠️ The waiting is the point: `main` drops the runtime on return, destroying a tick in flight;
    /// the grace bounds it.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        let health = self.health;
        let mut metadata_task = spawn_metadata_worker(
            Arc::clone(&self.token_metadata_repository),
            self.metadata_source.clone(),
            self.poll_interval,
            Arc::clone(&health),
            shutdown.clone(),
        );
        let mut price_task = spawn_price_worker(
            Arc::clone(&self.token_metadata_repository),
            Arc::clone(&self.token_price_repository),
            self.price_source.clone(),
            self.price_interval,
            shutdown.clone(),
        );
        // At the metadata cadence: it fills the mints metadata and price enrichment key off.
        let mut pool_account_task = spawn_pool_account_worker(
            self.pool_account_resolvers.clone(),
            self.pool_repository.clone(),
            self.pool_account_source.clone(),
            self.poll_interval,
            Arc::clone(&health),
            shutdown.clone(),
        );
        let mut reporter_task = self
            .heartbeat
            .clone()
            .map(|heartbeat| spawn_heartbeat_reporter(health, heartbeat, shutdown.clone()));

        // ⚠️ The cancellation arm carries no verdict: a failing worker may still be stopping, and its
        // error arrives through `Stop`; `ended` lets it step over the handle the `select!` polled.
        let (ended, first) = tokio::select! {
            result = &mut metadata_task => (Some(METADATA), handle_task_result(result, METADATA)),
            result = &mut price_task => (Some(PRICE), handle_task_result(result, PRICE)),
            result = &mut pool_account_task => {
                (Some(POOL_ACCOUNT), handle_task_result(result, POOL_ACCOUNT))
            }
            result = reporter_end(&mut reporter_task) => {
                (Some(REPORTER), handle_task_result(result, REPORTER))
            }
            _ = shutdown.cancelled() => {
                log::cancellation_received();
                (None, Ok(()))
            }
        };

        // Whichever arm fired, the others are told. Idempotent.
        shutdown.cancel();

        // ⚠️ The price worker first: `Stop` spends one grace across the tasks, and only its work is
        // lost; the other two re-list what they did not finish.
        let mut stop = Stop::new(first, ended);
        stop.settle(PRICE, &mut price_task).await;
        stop.settle(METADATA, &mut metadata_task).await;
        stop.settle(POOL_ACCOUNT, &mut pool_account_task).await;
        if let Some(task) = &mut reporter_task {
            stop.settle(REPORTER, task).await;
        }

        stop.finish()
    }
}

/// The names the four tasks answer to, in the logs and in the list of what outlived the grace.
const METADATA: &str = "metadata worker";
const PRICE: &str = "price worker";
const POOL_ACCOUNT: &str = "pool-account worker";
const REPORTER: &str = "heartbeat reporter";

async fn reporter_end(
    task: &mut Option<JoinHandle<Result<(), WorkerError>>>,
) -> Result<Result<(), WorkerError>, JoinError> {
    match task {
        Some(task) => task.await,
        None => std::future::pending().await,
    }
}

fn spawn_heartbeat_reporter(
    health: Arc<SourceHealth>,
    heartbeat: Arc<dyn Heartbeat>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let reporter = HeartbeatReporter::new(health, heartbeat);
    tokio::spawn(async move { reporter.run(shutdown).await })
}

/// Spawn the metadata worker task.
fn spawn_metadata_worker(
    repository: Arc<dyn TokenMetadataRepository>,
    metadata_source: Arc<dyn MetadataSource>,
    poll_interval: std::time::Duration,
    health: Arc<SourceHealth>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = MetadataWorker::new(repository, metadata_source, poll_interval, health);
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

/// Spawn the pool-account resolver worker task.
fn spawn_pool_account_worker(
    resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    pool_repository: Arc<dyn PoolRepository>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
    health: Arc<SourceHealth>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = PoolAccountWorker::new(resolvers, pool_repository, source, poll_interval, health);
    tokio::spawn(async move { worker.run(shutdown).await })
}
