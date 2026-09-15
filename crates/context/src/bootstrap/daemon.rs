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
use yog_bootstrap::{SecretUrl, Stop, handle_task_result};
use yog_core::domain::{
    PoolAccountResolver, PoolRepository, TokenMetadataRepository, TokenPriceRepository,
};
use yog_persistence::{
    Database, PgMeteoraDammV2PoolPropertiesRepository, PgMeteoraDlmmPoolPropertiesRepository,
    PgPoolRepository, PgTokenMetadataRepository, PgTokenPriceRepository,
};

use crate::bootstrap::Config;
use crate::error::WorkerError;
use crate::providers::ProviderMetrics;
use crate::providers::{HeliusDasClient, JupiterPriceClient, SolanaAccountClient};
use crate::source::{MetadataSource, PoolAccountSource, PriceSource};
use crate::workers::MetadataWorkerMetrics;
use crate::workers::PriceWorkerMetrics;
use crate::workers::{MetadataWorker, PoolAccountWorker, PriceWorker};

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
    /// Context METADATA poll secs
    poll_interval: std::time::Duration,
    /// context PRICE interval secs
    price_interval: std::time::Duration,
}

impl Daemon {
    /// Connect to the database, build the repositories and the source
    /// clients.
    pub(crate) async fn new(config: &Config) -> anyhow::Result<Self> {
        let database = init_db(&config.database_url)
            .await
            .context("database initialization failed")?;
        info!("database initialized");

        let poll_interval = config.metadata_poll_interval;
        let price_interval = config.price_interval;

        let db_pool = database.pool().clone();

        let token_metadata_repository: Arc<dyn TokenMetadataRepository> =
            Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));

        let token_price_repository: Arc<dyn TokenPriceRepository> =
            Arc::new(PgTokenPriceRepository::new(db_pool.clone()));

        // One resolver per protocol. Each owns its own enrichment queue and its
        // own tables; the worker iterates and names none of them. Adding DLMM
        // means pushing its resolver here — nothing else moves.
        let pool_account_resolvers: Vec<Arc<dyn PoolAccountResolver>> = vec![
            Arc::new(PgMeteoraDammV2PoolPropertiesRepository::new(
                db_pool.clone(),
            )),
            Arc::new(PgMeteoraDlmmPoolPropertiesRepository::new(db_pool.clone())),
        ];
        // Written by the same worker, from the same account read — but through
        // the repository that owns `pools`, so no satellite is a co-writer of
        // the cross-protocol registry.
        let pool_repository: Arc<dyn PoolRepository> = Arc::new(PgPoolRepository::new(db_pool));

        // Two independent HTTP clients — one per external source. Each takes
        // the wrapped secret, not the exposed string: the type travels to the
        // request builder, so `.expose()` never happens this far from the wire.
        let metadata_source = Arc::new(HeliusDasClient::new(config.token_metadata.url()));
        let price_source = Arc::new(JupiterPriceClient::new(
            config.jupiter_url.clone(),
            config.jupiter_api_key.clone(),
        ));
        // `getMultipleAccounts`, standard Solana JSON-RPC — its own endpoint,
        // which may or may not be the provider serving DAS above. Logged
        // side by side because that is what makes the split visible at
        // startup rather than in a config file nobody rereads.
        let pool_account_source: Arc<dyn PoolAccountSource> =
            Arc::new(SolanaAccountClient::new(config.pool_account.url()));
        info!(
            token_metadata = %config.token_metadata,
            pool_account = %config.pool_account,
            "external endpoints initialized"
        );

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
    /// this returns, and a dropped runtime destroys whatever is still in
    /// flight. Until 14 September 2026 the `ctrl_c` arm of the `select!` below
    /// returned `Ok(())` on its own and the process was gone 5–7 ms later:
    /// measured over 20 stops on `main`, **none** saw the three workers hand
    /// back, and the ten that fell inside a price tick destroyed it ten times
    /// out of ten — 645–747 `token_prices` rows, timestamped, that the next
    /// cycle does not redo.
    ///
    /// The grace is what keeps the wait from becoming a hang: a worker that
    /// will not end is named in the logs and left to the runtime. The same
    /// measurement says that will happen — a price tick takes 10.7–19.9 s
    /// against a rate-limiting Jupiter, far past
    /// [`yog_bootstrap::SHUTDOWN_GRACE`], and 10 stops out of 10 taken inside
    /// one lost it.
    ///
    /// ⚠️ **That is not a missing timeout.** Every provider request is already
    /// bounded (15 s total, 5 s connect — `providers::http_client`). The tick
    /// is long because it is ~19 chunks sent back to back plus the capped
    /// backoff the rate-limited ones earn, and **nothing between two chunks
    /// looks at the token**. Shortening it is a question for the worker and
    /// its client, not for the grace.
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

        // ⚠️ **The cancellation arm carries no verdict.** It says the stop was
        // asked for, not what happened, and a worker that failed can still be
        // in the middle of stopping when it fires — so its error arrives
        // afterwards, through the drain. `Stop` is what collects it.
        //
        // Which worker answered comes back with its outcome, because `Stop`
        // needs it: that handle has been polled to completion, and the drain
        // below has to step over it. The rule lives in `settle`, not here.
        let (ended, first) = tokio::select! {
            result = &mut metadata_task => (Some(METADATA), handle_task_result(result, METADATA)),
            result = &mut price_task => (Some(PRICE), handle_task_result(result, PRICE)),
            result = &mut pool_account_task => {
                (Some(POOL_ACCOUNT), handle_task_result(result, POOL_ACCOUNT))
            }
            _ = shutdown.cancelled() => {
                info!("cancellation received — stopping");
                (None, Ok(()))
            }
        };

        // Whichever arm fired, the other two have to be told. Idempotent, so
        // the cancellation arm — whose token is already cancelled — needs no
        // branch of its own.
        shutdown.cancel();

        // ⚠️ **The price worker is waited on first, and the order is a
        // decision.** `Stop` spends one grace across the three, so the first
        // one waited on can eat all of it — and the price worker is the only
        // one whose interrupted work is *lost* rather than merely *abandoned*.
        // Its tick ends in a single `INSERT` of prices stamped at one instant;
        // destroy it and that instant is gone. The other two re-list what they
        // did not finish on their next tick (`list_missing_mints`,
        // `list_unresolved`), so interrupting them costs time, not rows.
        let mut stop = Stop::new(first, ended);
        stop.settle(PRICE, &mut price_task).await;
        stop.settle(METADATA, &mut metadata_task).await;
        stop.settle(POOL_ACCOUNT, &mut pool_account_task).await;

        stop.finish()
    }
}

/// The names the three workers answer to — in the logs, and in the list of
/// what outlived the grace. Named once because each is written at two sites.
const METADATA: &str = "metadata worker";
const PRICE: &str = "price worker";
const POOL_ACCOUNT: &str = "pool-account worker";

/// Connect to the database.
///
/// The database URL is held in `Config::database_url` (a redacted secret),
/// so we never log it directly — `anyhow::Context` is sufficient to surface
/// the failure at startup without leaking credentials.
async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    let db = Database::connect(database_url.expose())
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
