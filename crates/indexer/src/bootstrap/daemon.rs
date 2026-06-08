use crate::{
    application::{
        reporter::{NetworkStatusReporter, NetworkStatusReporterError},
        services::{
            EventPersistor, EventPersistorMetrics, IndexerService, IndexerServiceMetrics,
            WatchedPoolService,
        },
        workers::IndexerWorker,
    },
    bootstrap::Config,
    error::{DispatcherError, IndexerWorkerError, RpcListenerError},
    infra::{
        RpcListener,
        rpc::{
            DispatcherMetrics, QualifiedSignature, RawLogEvent, SignatureDispatcher,
            TransactionFetcher,
        },
    },
    utils::redact_api_key,
};
use anyhow::Context;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_persistence::{
    Database, PgClaimPositionFeeEventRepository, PgClaimRewardEventRepository,
    PgLiquidityEventRepository, PgNetworkStatusRepository, PgPoolCurrentStateRepository,
    PgPoolRepository, PgSwapEventRepository, PgWatchedPoolRepository,
};

/// Top-level process — owns all runtime dependencies and drives the
/// indexer lifecycle.
///
/// Responsibilities:
/// - initialise all dependencies (database, RPC client, services)
/// - register the observed protocols at startup
/// - run the WebSocket listener, the dispatcher and the indexer worker
/// - handle graceful shutdown on SIGTERM / Ctrl-C
pub(crate) struct Daemon {
    indexer_service: Arc<IndexerService>,
    watched_pool_service: Arc<WatchedPoolService>,
    listener: Arc<RpcListener>,
    dispatcher: SignatureDispatcher,
    network_status_reporter: NetworkStatusReporter,
    _database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    ///
    /// Fails fast if the database is unreachable, if migrations cannot
    /// be applied, or if the dispatcher is misconfigured.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let database = init_db(config.database_url.expose())
            .await
            .context("database initialization failed")?;
        info!("database initialized");

        let listener = init_listener(&config);
        info!("RPC listener initialized: {}", config.solana_rpc_ws);

        let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.expose().to_string()));
        info!("RPC HTTP client initialized: {}", config.solana_rpc_http);

        let indexer_service = init_indexer_service(&database, rpc_client.clone())
            .await
            .context("indexer service initialization failed")?;
        info!("indexer service initialized");

        let network_status_reporter = init_network_status_reporter(&database, rpc_client.clone())
            .await
            .context("network_status_reporter initialization failed")?;

        let watched_pool_service = init_watched_pool_service(&database, listener.clone())
            .await
            .context("watched pool service initialization failed")?;
        info!("watched pool service initialized");

        let dispatcher =
            SignatureDispatcher::new_default().context("dispatcher initialization failed")?;
        info!("dispatcher initialized");

        DispatcherMetrics::register_descriptions();
        IndexerServiceMetrics::register_descriptions();
        EventPersistorMetrics::register_descriptions();
        info!("Metrics initialized");

        info!("daemon initialized");

        Ok(Self {
            indexer_service,
            watched_pool_service,
            listener,
            dispatcher,
            network_status_reporter,
            _database: database,
        })
    }

    /// Start the daemon. Consumes `self` — cannot be called twice.
    ///
    /// Spawns three tasks connected by bounded channels:
    ///
    /// ```text
    /// listener → (RawLogEvent) → dispatcher → (QualifiedSignature) → indexer worker
    /// ```
    ///
    /// Returns as soon as any task fails or the shutdown token is
    /// triggered. All remaining tasks are cancelled via the shared
    /// token.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        self.watched_pool_service.restore_subscriptions().await?;
        let (raw_tx, raw_rx) = mpsc::channel::<RawLogEvent>(10_000);
        let (sig_tx, sig_rx) = mpsc::channel::<QualifiedSignature>(10_000);

        let ws_task = spawn_websocket_task(Arc::clone(&self.listener), raw_tx, shutdown.clone());
        let dispatcher_task =
            spawn_dispatcher_task(self.dispatcher, raw_rx, sig_tx, shutdown.clone());
        let indexer_task =
            spawn_indexer_task(Arc::clone(&self.indexer_service), sig_rx, shutdown.clone());

        let reporter_task =
            spawn_network_status_reporter_task(self.network_status_reporter, shutdown.clone());

        tokio::select! {
            result = ws_task => {
                shutdown.cancel();
                handle_task_result(result, "WebSocket listener")?
            }
            result = dispatcher_task => {
                shutdown.cancel();
                handle_task_result(result, "dispatcher")?
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

// ── Initialisation helpers ───────────────────────────────────────────────────

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

/// Create the RPC WebSocket listener with its watched protocols.
fn init_listener(config: &Config) -> Arc<RpcListener> {
    Arc::new(RpcListener::new(
        config.solana_rpc_ws.expose().to_string(),
        config.worker_max_retries,
        config.mode_protocol_centric,
    ))
}

/// Build the EventPersistor with its six repositories.
fn init_event_persistor(database: &Database) -> Arc<EventPersistor> {
    let pg_swap_event_repo = Arc::new(PgSwapEventRepository::new(database.pool().clone()));
    let pg_liquidity_event_repo =
        Arc::new(PgLiquidityEventRepository::new(database.pool().clone()));
    let pg_claim_position_fee_repo = Arc::new(PgClaimPositionFeeEventRepository::new(
        database.pool().clone(),
    ));
    let pg_claim_reward_repo = Arc::new(PgClaimRewardEventRepository::new(database.pool().clone()));
    let pg_pool_repo = Arc::new(PgPoolRepository::new(database.pool().clone()));
    let pg_pool_current_state_repo =
        Arc::new(PgPoolCurrentStateRepository::new(database.pool().clone()));

    Arc::new(EventPersistor::new(
        pg_swap_event_repo,
        pg_liquidity_event_repo,
        pg_claim_position_fee_repo,
        pg_claim_reward_repo,
        pg_pool_repo,
        pg_pool_current_state_repo,
    ))
}

/// Initialise the indexer service and its repository dependencies.
async fn init_indexer_service(
    database: &Database,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<Arc<IndexerService>> {
    let event_persistor = init_event_persistor(database);
    info!("event persistor initialized");
    let transaction_fetcher = Arc::new(TransactionFetcher::new(rpc_client.clone()));
    info!("transaction fetcher initialized");

    let indexer_service = Arc::new(IndexerService::new(
        Arc::clone(&event_persistor),
        Arc::clone(&transaction_fetcher),
    ));
    info!("indexer service initialized");

    Ok(indexer_service)
}

/// Initialise the NetworkStautsReporter and its repository dependency
async fn init_network_status_reporter(
    database: &Database,
    rpc_client: Arc<RpcClient>,
) -> anyhow::Result<NetworkStatusReporter> {
    let pg_network_status_reporter_repository =
        Arc::new(PgNetworkStatusRepository::new(database.pool().clone()));
    Ok(NetworkStatusReporter::new(
        rpc_client,
        pg_network_status_reporter_repository,
    ))
}

// Initialise the WatchedPoolService and its repository dependency.
async fn init_watched_pool_service(
    database: &Database,
    listener: Arc<RpcListener>,
) -> anyhow::Result<Arc<WatchedPoolService>> {
    let pg_watched_pool_repository =
        Arc::new(PgWatchedPoolRepository::new(database.pool().clone()));
    Ok(Arc::new(WatchedPoolService::new(
        listener,
        pg_watched_pool_repository,
    )))
}
// ── Task spawners ────────────────────────────────────────────────────────────

/// Spawn the WebSocket listener task.
fn spawn_websocket_task(
    listener: Arc<RpcListener>,
    tx: mpsc::Sender<RawLogEvent>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), RpcListenerError>> {
    tokio::spawn(async move { listener.run(tx, shutdown).await })
}

/// Spawn the dispatcher task.
fn spawn_dispatcher_task(
    dispatcher: SignatureDispatcher,
    raw_rx: mpsc::Receiver<RawLogEvent>,
    sig_tx: mpsc::Sender<QualifiedSignature>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), DispatcherError>> {
    tokio::spawn(async move { dispatcher.run(raw_rx, sig_tx, shutdown).await })
}

/// Spawn the indexer worker task.
///
/// Per-signature failures stay inside the worker (logged, counted, not
/// propagated). Only loop-level failures reach the returned `JoinHandle`
/// and bubble up to `Daemon::run`.
fn spawn_indexer_task(
    indexer_service: Arc<IndexerService>,
    rx: mpsc::Receiver<QualifiedSignature>,
    shutdown: CancellationToken,
) -> JoinHandle<Result<(), IndexerWorkerError>> {
    let worker = IndexerWorker::new(indexer_service);
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
            let msg = redact_api_key(&e.to_string());
            tracing::error!(error = %msg, "{task_name} failed");
            Err(anyhow::Error::new(e))
        }
        Err(e) => {
            let msg = redact_api_key(&e.to_string());
            tracing::error!(error = %msg, "{task_name} panicked");
            Err(anyhow::anyhow!("{task_name} panicked: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_task_result_clean_stop_returns_ok() {
        let result: Result<Result<(), std::io::Error>, tokio::task::JoinError> = Ok(Ok(()));
        assert!(handle_task_result(result, "test task").is_ok());
    }

    #[test]
    fn handle_task_result_task_error_returns_err() {
        let err = std::io::Error::other("boom");
        let result: Result<Result<(), std::io::Error>, tokio::task::JoinError> = Ok(Err(err));
        assert!(handle_task_result(result, "test task").is_err());
    }
}
