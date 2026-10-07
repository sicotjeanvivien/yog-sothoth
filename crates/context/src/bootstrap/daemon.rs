//! Shared daemon dependencies, assembled once at startup: the repositories
//! and the three source clients, each with its own HTTP client.

use std::sync::Arc;

use anyhow::Context;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::{SecretUrl, Stop, handle_task_result};
use yog_core::domain::{
    PoolAccountResolver, PoolRepository, TokenMetadataRepository, TokenPriceRepository,
};
use yog_persistence::{
    Database, PgMeteoraDammV2PoolPropertiesRepository, PgMeteoraDlmmPoolPropertiesRepository,
    PgPoolRepository, PgTokenMetadataRepository, PgTokenPriceRepository,
};

use crate::application::source::{MetadataSource, PoolAccountSource, PriceSource};
use crate::application::workers::{
    MetadataWorker, MetadataWorkerMetrics, PoolAccountWorker, PriceWorker, PriceWorkerMetrics,
};
use crate::bootstrap::Config;
use crate::error::WorkerError;
use crate::infra::{HeliusDasClient, JupiterPriceClient, ProviderMetrics, SolanaAccountClient};

mod log;

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
}

impl Daemon {
    /// Connect to the database, build the repositories and the source
    /// clients.
    pub(crate) async fn new(config: &Config) -> anyhow::Result<Self> {
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

        // One resolver per protocol, each with its own queue and tables; the
        // worker names none of them. A new protocol pushes its resolver here.
        let pool_account_resolvers: Vec<Arc<dyn PoolAccountResolver>> = vec![
            Arc::new(PgMeteoraDammV2PoolPropertiesRepository::new(
                db_pool.clone(),
            )),
            Arc::new(PgMeteoraDlmmPoolPropertiesRepository::new(db_pool.clone())),
        ];
        // Same worker, same account read, but through the repository that owns
        // `pools`: no satellite writes the cross-protocol registry.
        let pool_repository: Arc<dyn PoolRepository> = Arc::new(PgPoolRepository::new(db_pool));

        // Each client takes the wrapped secret: `.expose()` happens at the
        // request builder, never here.
        let metadata_source = Arc::new(HeliusDasClient::new(config.token_metadata.url()));
        let price_source = Arc::new(JupiterPriceClient::new(
            config.jupiter_url.clone(),
            config.jupiter_api_key.clone(),
            config.jupiter_rate_limit,
        ));
        log::jupiter_spacing(config.jupiter_rate_limit, price_source.request_spacing());
        // `getMultipleAccounts`, its own endpoint, which may or may not be the
        // provider serving DAS: the two are logged side by side, so that the
        // split shows at startup.
        let pool_account_source: Arc<dyn PoolAccountSource> =
            Arc::new(SolanaAccountClient::new(config.pool_account.url()));
        log::endpoints_initialized(&config.token_metadata, &config.pool_account);

        MetadataWorkerMetrics::register_descriptions();
        PriceWorkerMetrics::register_descriptions();
        ProviderMetrics::register_descriptions();

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
        })
    }

    /// Run the three workers until one of them ends or the stop is asked for,
    /// then **wait for the others** before returning.
    ///
    /// ⚠️ **The waiting is the point.** `main` drops the runtime the moment
    /// this returns, destroying whatever is still in flight — a price tick
    /// among them. The grace bounds the wait: a worker that will not end is
    /// named in the logs and left behind.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        let mut metadata_task = spawn_metadata_worker(
            Arc::clone(&self.token_metadata_repository),
            self.metadata_source.clone(),
            self.poll_interval,
            shutdown.clone(),
        );
        let mut price_task = spawn_price_worker(
            Arc::clone(&self.token_metadata_repository),
            Arc::clone(&self.token_price_repository),
            self.price_source.clone(),
            self.price_interval,
            shutdown.clone(),
        );
        // Resolver runs at the metadata cadence — it must fill mints + fee before
        // metadata/price enrichment has anything to key off.
        let mut pool_account_task = spawn_pool_account_worker(
            self.pool_account_resolvers.clone(),
            self.pool_repository.clone(),
            self.pool_account_source.clone(),
            self.poll_interval,
            shutdown.clone(),
        );

        // ⚠️ The cancellation arm carries no verdict: a worker that failed can
        // still be stopping when it fires, and its error arrives through the
        // drain. The worker that ended comes back with its outcome, so that
        // `Stop` steps over its handle, already polled to completion.
        let (ended, first) = tokio::select! {
            result = &mut metadata_task => (Some(METADATA), handle_task_result(result, METADATA)),
            result = &mut price_task => (Some(PRICE), handle_task_result(result, PRICE)),
            result = &mut pool_account_task => {
                (Some(POOL_ACCOUNT), handle_task_result(result, POOL_ACCOUNT))
            }
            _ = shutdown.cancelled() => {
                log::cancellation_received();
                (None, Ok(()))
            }
        };

        // Whichever arm fired, the others are told. Idempotent.
        shutdown.cancel();

        // ⚠️ The price worker is waited on first: `Stop` spends one grace
        // across the three, and only its interrupted work is lost — prices
        // stamped at an instant no later tick redoes. The other two re-list
        // what they did not finish (`list_missing_mints`, `list_unresolved`).
        let mut stop = Stop::new(first, ended);
        stop.settle(PRICE, &mut price_task).await;
        stop.settle(METADATA, &mut metadata_task).await;
        stop.settle(POOL_ACCOUNT, &mut pool_account_task).await;

        stop.finish()
    }
}

/// The names the three workers answer to, in the logs and in the list of what
/// outlived the grace.
const METADATA: &str = "metadata worker";
const PRICE: &str = "price worker";
const POOL_ACCOUNT: &str = "pool-account worker";

/// Connect to the database. The error names the failure, never the URL.
async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    let db = Database::connect(database_url.expose())
        .await
        .context("failed to connect to database")?;
    log::connected_to_database();
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

/// Spawn the pool-account resolver worker task.
fn spawn_pool_account_worker(
    resolvers: Vec<Arc<dyn PoolAccountResolver>>,
    pool_repository: Arc<dyn PoolRepository>,
    source: Arc<dyn PoolAccountSource>,
    poll_interval: std::time::Duration,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), WorkerError>> {
    let worker = PoolAccountWorker::new(resolvers, pool_repository, source, poll_interval);
    tokio::spawn(async move { worker.run(shutdown).await })
}
