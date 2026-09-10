use crate::{
    application::{
        reporter::{NetworkStatusReporter, NetworkStatusReporterError},
        services::{
            DammV2Repos, EventPersistor, EventPersistorMetrics, MeteoraDammV2EventPersistor,
            PoolMaintenance, TransactionProcessor, TransactionProcessorMetrics, WatchedPoolService,
        },
        source::{IngestedTransaction, TransactionSource},
        workers::IndexerWorker,
    },
    bootstrap::Config,
    error::{IndexerWorkerError, SourceError},
    infra::{
        DispatcherMetrics, FetchMetrics, GrpcBufferMetrics, GrpcListenerMetrics, RpcListener,
        RpcTransactionSource, SignatureDispatcher, TransactionFetcher,
    },
};
use anyhow::Context;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_bootstrap::SecretUrl;
use yog_core::application::extraction::ExtractionDispatcher;
use yog_persistence::{
    Database, PgMeteoraDammV2ClaimPositionFeeEventRepository,
    PgMeteoraDammV2ClaimProtocolFeeEventRepository, PgMeteoraDammV2ClaimRewardEventRepository,
    PgMeteoraDammV2ClosePositionEventRepository, PgMeteoraDammV2CreatePositionEventRepository,
    PgMeteoraDammV2FundRewardEventRepository, PgMeteoraDammV2InitializePoolEventRepository,
    PgMeteoraDammV2InitializeRewardEventRepository, PgMeteoraDammV2LiquidityEventRepository,
    PgMeteoraDammV2LockPositionEventRepository,
    PgMeteoraDammV2PermanentLockPositionEventRepository,
    PgMeteoraDammV2SetPoolStatusEventRepository, PgMeteoraDammV2SplitPositionEventRepository,
    PgMeteoraDammV2SwapEventRepository, PgMeteoraDammV2UpdatePoolFeesEventRepository,
    PgMeteoraDammV2UpdateRewardDurationEventRepository,
    PgMeteoraDammV2UpdateRewardFunderEventRepository,
    PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    PgMeteoraDammV2WithdrawIneligibleRewardEventRepository, PgNetworkStatusRepository,
    PgPoolCurrentStateRepository, PgPoolRepository, PgWatchedPoolRepository,
};

/// Top-level process — owns all runtime dependencies and drives the
/// indexer lifecycle.
///
/// Responsibilities:
/// - initialise all dependencies (database, RPC client, services)
/// - register the observed protocols at startup
/// - run the transaction source and the indexer worker
/// - handle graceful shutdown on SIGTERM / Ctrl-C
///
/// It is the **composition root**, and the only place that knows which
/// acquisition model is running: it builds one [`TransactionSource`] and
/// everything downstream sees the trait.
pub(crate) struct Daemon {
    processor: Arc<TransactionProcessor>,
    watched_pool_service: Arc<WatchedPoolService>,
    source: Arc<dyn TransactionSource>,
    network_status_reporter: NetworkStatusReporter,
    /// How many transactions may be persisted at once — computed from the pool
    /// that was actually opened, see [`index_concurrency`].
    index_concurrency: usize,
    _database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    ///
    /// Fails fast if the database is unreachable, if migrations cannot
    /// be applied, or if the dispatcher is misconfigured.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let database = init_db(&config.database_url)
            .await
            .context("database initialization failed")?;
        info!("database initialized");

        let rpc_client = Arc::new(RpcClient::new(
            config.ingest_transaction.url().expose().to_string(),
        ));
        info!(
            "transaction RPC client initialized: {}",
            config.ingest_transaction
        );

        let source = init_source(&config, rpc_client.clone())
            .context("transaction source initialization failed")?;
        info!("transaction source initialized: {}", config.ingest_stream);

        let processor = init_processor(&database);
        info!("indexer service initialized");

        let network_status_reporter = init_network_status_reporter(
            &database,
            rpc_client.clone(),
            config.ingest_transaction.url(),
        )
        .await
        .context("network_status_reporter initialization failed")?;

        let watched_pool_service = init_watched_pool_service(&database, Arc::clone(&source))
            .await
            .context("watched pool service initialization failed")?;
        info!("watched pool service initialized");

        DispatcherMetrics::register_descriptions();
        FetchMetrics::register_descriptions();
        TransactionProcessorMetrics::register_descriptions();
        EventPersistorMetrics::register_descriptions();
        // The gRPC path's two families are registered whichever source is
        // running. Descriptions are only HELP text — registering them costs a
        // string and exports nothing until a counter is touched — and the
        // alternative, registering them where the gRPC listener is built, is a
        // line that only ever runs on the path whose reader has the least
        // context. A counter exported without its HELP text is unreadable to
        // exactly the person who goes looking for it.
        GrpcBufferMetrics::register_descriptions();
        GrpcListenerMetrics::register_descriptions();
        info!("Metrics initialized");

        info!("daemon initialized");

        let index_concurrency = index_concurrency(&database)?;
        info!(index_concurrency, "index concurrency derived from the pool");

        Ok(Self {
            processor,
            watched_pool_service,
            source,
            network_status_reporter,
            index_concurrency,
            _database: database,
        })
    }

    /// Start the daemon. Consumes `self` — cannot be called twice.
    ///
    /// Spawns three tasks, and the ingestion half of the graph is **one edge**:
    ///
    /// ```text
    /// source → (IngestedTransaction) → indexer worker
    /// ```
    ///
    /// Whatever a source needs to produce that — a fleet of WebSockets, a
    /// filter chain and a fetch stage on one path, a single stream on the
    /// other — it owns and supervises itself. This graph does not change when
    /// the source does, which is the point of the port.
    ///
    /// Returns as soon as any task fails or the shutdown token is
    /// triggered. All remaining tasks are cancelled via the shared
    /// token.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        self.watched_pool_service.restore_subscriptions().await?;
        let (tx, rx) = mpsc::channel::<IngestedTransaction>(INGESTED_CHANNEL_CAPACITY);

        let source_task = spawn_source_task(Arc::clone(&self.source), tx, shutdown.clone());
        let indexer_task = spawn_indexer_task(
            Arc::clone(&self.processor),
            rx,
            self.index_concurrency,
            shutdown.clone(),
        );
        let reporter_task =
            spawn_network_status_reporter_task(self.network_status_reporter, shutdown.clone());

        tokio::select! {
            result = source_task => {
                shutdown.cancel();
                handle_task_result(result, "transaction source")?
            }
            result = indexer_task => {
                shutdown.cancel();
                handle_task_result(result, "indexer worker")?
            }
            result = reporter_task => {
                shutdown.cancel();
                handle_task_result(result, "network status reporter")?
            }
            _ = shutdown.cancelled() => tracing::info!("cancellation received — stopping"),
        }
        Ok(())
    }
}

/// How many delivered transactions may queue before the source is made to wait.
///
/// ⚠️ **Ten times smaller than the channels it replaces, and on purpose.** What
/// queued between the old stages was a signature and a little text, so 10 000
/// of them cost nothing. What queues here is a whole transaction — 5–20 kB
/// each — so the same figure would be 50–200 MB in this channel alone, on top
/// of the gRPC path's own pending buffer. 1 000 is 5–20 MB and still leaves two
/// orders of magnitude more room than the worker's concurrency bound, which is
/// what actually drains it.
const INGESTED_CHANNEL_CAPACITY: usize = 1_000;

// ── Initialisation helpers ───────────────────────────────────────────────────

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

/// Build the transaction source the configuration selects.
///
/// ⚠️ **One arm today, and `INGEST_SOURCE` is still not read here.** The gRPC
/// source is written and reachable by nothing — `check_supported` refuses
/// `INGEST_SOURCE=grpc` at load time, and lifting that refusal is the next
/// slice of `03 - active/listener-grpc-yellowstone.md`. This function is the
/// single place that will grow the second arm, and the only place in the crate
/// that will ever name a concrete source.
fn init_source(
    config: &Config,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Arc<dyn TransactionSource>> {
    let listener = Arc::new(RpcListener::new(
        config.ingest_stream.clone(),
        config.worker_max_retries,
        config.scope,
    ));
    let dispatcher =
        Arc::new(SignatureDispatcher::new_default().context("dispatcher initialization failed")?);
    let fetcher = Arc::new(TransactionFetcher::new(
        rpc_client,
        config.ingest_transaction.url(),
    ));

    Ok(Arc::new(RpcTransactionSource::new(
        listener, dispatcher, fetcher,
    )))
}

/// Build the EventPersistor: shared pool maintenance plus the per-protocol
/// sub-persistor and its bundle of per-event-kind repositories.
fn init_event_persistor(database: &Database) -> Arc<EventPersistor> {
    // Cross-protocol repositories
    let pg_pool_repo = Arc::new(PgPoolRepository::new(database.pool().clone()));
    let pg_pool_current_state_repo =
        Arc::new(PgPoolCurrentStateRepository::new(database.pool().clone()));

    // Shared pool maintenance helper — reused by every per-protocol sub-persistor.
    let pool_maintenance = Arc::new(PoolMaintenance::new(
        pg_pool_repo,
        pg_pool_current_state_repo,
    ));

    // Meteora DAMM v2 sub-persistor and its per-event-kind repositories.
    let pool = || database.pool().clone();
    let damm_v2_repos = DammV2Repos {
        swap_event: Arc::new(PgMeteoraDammV2SwapEventRepository::new(pool())),
        liquidity_event: Arc::new(PgMeteoraDammV2LiquidityEventRepository::new(pool())),
        claim_position_fee: Arc::new(PgMeteoraDammV2ClaimPositionFeeEventRepository::new(pool())),
        claim_protocol_fee: Arc::new(PgMeteoraDammV2ClaimProtocolFeeEventRepository::new(pool())),
        claim_reward: Arc::new(PgMeteoraDammV2ClaimRewardEventRepository::new(pool())),
        initialize_reward: Arc::new(PgMeteoraDammV2InitializeRewardEventRepository::new(pool())),
        fund_reward: Arc::new(PgMeteoraDammV2FundRewardEventRepository::new(pool())),
        withdraw_ineligible_reward: Arc::new(
            PgMeteoraDammV2WithdrawIneligibleRewardEventRepository::new(pool()),
        ),
        update_reward_duration: Arc::new(PgMeteoraDammV2UpdateRewardDurationEventRepository::new(
            pool(),
        )),
        update_reward_funder: Arc::new(PgMeteoraDammV2UpdateRewardFunderEventRepository::new(
            pool(),
        )),
        withdraw_dead_liquidity_reward: Arc::new(
            PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository::new(pool()),
        ),
        create_position: Arc::new(PgMeteoraDammV2CreatePositionEventRepository::new(pool())),
        close_position: Arc::new(PgMeteoraDammV2ClosePositionEventRepository::new(pool())),
        lock_position: Arc::new(PgMeteoraDammV2LockPositionEventRepository::new(pool())),
        permanent_lock_position: Arc::new(
            PgMeteoraDammV2PermanentLockPositionEventRepository::new(pool()),
        ),
        initialize_pool: Arc::new(PgMeteoraDammV2InitializePoolEventRepository::new(pool())),
        set_pool_status: Arc::new(PgMeteoraDammV2SetPoolStatusEventRepository::new(pool())),
        split_position: Arc::new(PgMeteoraDammV2SplitPositionEventRepository::new(pool())),
        update_pool_fees: Arc::new(PgMeteoraDammV2UpdatePoolFeesEventRepository::new(pool())),
    };

    let meteora_damm_v2 = Arc::new(MeteoraDammV2EventPersistor::new(
        damm_v2_repos,
        Arc::clone(&pool_maintenance),
    ));

    Arc::new(EventPersistor::new(meteora_damm_v2))
}

/// Initialise the indexer service and its repository dependencies.
fn init_processor(database: &Database) -> Arc<TransactionProcessor> {
    let extraction_dispatcher = Arc::new(ExtractionDispatcher::new());
    info!("event extractor initialized");
    let event_persistor = init_event_persistor(database);
    info!("event persistor initialized");

    Arc::new(TransactionProcessor::new(
        extraction_dispatcher,
        event_persistor,
    ))
}

/// Initialise the NetworkStautsReporter and its repository dependency
async fn init_network_status_reporter(
    database: &Database,
    rpc_client: Arc<RpcClient>,
    rpc_url: SecretUrl,
) -> anyhow::Result<NetworkStatusReporter> {
    let pg_network_status_reporter_repository =
        Arc::new(PgNetworkStatusRepository::new(database.pool().clone()));
    Ok(NetworkStatusReporter::new(
        rpc_client,
        rpc_url,
        pg_network_status_reporter_repository,
    ))
}

// Initialise the WatchedPoolService and its repository dependency.
async fn init_watched_pool_service(
    database: &Database,
    source: Arc<dyn TransactionSource>,
) -> anyhow::Result<Arc<WatchedPoolService>> {
    let pg_watched_pool_repository =
        Arc::new(PgWatchedPoolRepository::new(database.pool().clone()));
    Ok(Arc::new(WatchedPoolService::new(
        source,
        pg_watched_pool_repository,
    )))
}
// ── Task spawners ────────────────────────────────────────────────────────────

/// Spawn the ingestion task — whatever shape the source's own graph has.
fn spawn_source_task(
    source: Arc<dyn TransactionSource>,
    tx: mpsc::Sender<IngestedTransaction>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move { source.run(tx, shutdown).await })
}

/// How many connections the pool's *other* in-process users may need while the
/// indexer worker is running.
///
/// One today: [`NetworkStatusReporter`] upserts a snapshot on every tick.
/// `WatchedPoolService` is not counted — it restores subscriptions once, before
/// the worker is spawned, and never touches the pool again.
const CONNECTIONS_RESERVED: u32 = 1;

/// How many transactions may be persisted concurrently.
///
/// Every task in flight holds a connection while it writes, so the ceiling is
/// the pool — but **not the whole pool**, and that distinction is the point.
/// Handing the worker all of it starves the reporter, whose `record_snapshot`
/// propagates with `?`: one `acquire_timeout` there returns `Err`,
/// `Daemon::run` takes the reporter branch, cancels the shared token, and the
/// process exits. A saturated database would stop the indexer by way of a
/// health probe, which is the least legible failure available.
///
/// Read from the pool that was opened rather than from
/// [`Database::DEFAULT_MAX_CONNECTIONS`], so that sizing the pool differently
/// resizes this too instead of silently parting ways with it.
///
/// # Errors
///
/// ⚠️ **Refuses a pool too small to reserve from, rather than clamping to one.**
/// The first version returned `.max(1)`, which defeated the reservation in the
/// exact case this function exists to prevent: a pool of one hands its only
/// connection to an index task while the reporter waits out `acquire_timeout`
/// and kills the process. Clamping cannot be right here — `Semaphore::new(0)`
/// would deadlock instead — so the only honest answers are "refuse" or "open a
/// bigger pool", and a configuration that cannot work should say so at startup
/// rather than five seconds into a busy minute.
fn index_concurrency(database: &Database) -> anyhow::Result<usize> {
    let max_connections = database.max_connections();
    anyhow::ensure!(
        max_connections > CONNECTIONS_RESERVED,
        "database pool holds {max_connections} connection(s), and {CONNECTIONS_RESERVED} must \
         stay free for the network status reporter — the indexer would have none left to \
         persist with. Open the pool with more connections."
    );
    Ok((max_connections - CONNECTIONS_RESERVED) as usize)
}

/// Spawn the indexer worker task.
///
/// Per-transaction failures stay inside the worker (logged, counted, not
/// propagated). Only loop-level failures reach the returned `JoinHandle`
/// and bubble up to `Daemon::run`.
fn spawn_indexer_task(
    processor: Arc<TransactionProcessor>,
    rx: mpsc::Receiver<IngestedTransaction>,
    max_concurrent: usize,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), IndexerWorkerError>> {
    let worker = IndexerWorker::new(processor, max_concurrent);
    tokio::spawn(async move { worker.run(rx, shutdown).await })
}

fn spawn_network_status_reporter_task(
    reporter: NetworkStatusReporter,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), NetworkStatusReporterError>> {
    tokio::spawn(async move { reporter.run(shutdown).await })
}

// ── Task result handling ─────────────────────────────────────────────────────

/// Normalise the result of a spawned task into a loggable anyhow::Result.
///
/// Distinguishes three cases: clean stop, task error, and task panic.
fn handle_task_result<E>(
    result: Result<Result<(), E>, tokio::task::JoinError>,
    task_name: &str,
) -> anyhow::Result<()>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match result {
        Ok(Ok(())) => {
            tracing::info!("{task_name} stopped");
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

#[cfg(test)]
#[path = "daemon_tests.rs"]
mod tests;
