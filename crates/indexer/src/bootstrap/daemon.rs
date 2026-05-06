use crate::{
    application::{
        services::{IndexerService, IndexerServiceMetrics, WatchedPoolService},
        workers::IndexerWorker,
    },
    config::Config,
    error::{DispatcherError, IndexerWorkerError, RpcListenerError},
    infra::{
        RpcListener,
        rpc::{
            QualifiedSignature, RawLogEvent, SignatureDispatcher,
            dispatcher::metrics::DispatcherMetrics,
        },
    },
    utils::redact_api_key,
};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_persistence::{
    Database, PgClaimPositionFeeEventRepository, PgClaimRewardEventRepository,
    PgLiquidityEventRepository, PgPoolRepository, PgSwapEventRepository, PgWatchedPoolRepository,
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
    _database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    ///
    /// Fails fast if the database is unreachable, if migrations cannot
    /// be applied, or if the dispatcher is misconfigured.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let database = init_db(&config.database_url.expose()).await?;
        info!("database initialized");

        let listener = init_listener(&config);
        info!("RPC listener initialized: {}", config.solana_rpc_ws);

        let indexer_service = init_indexer_service(&config, &database).await?;
        info!("indexer service initialized");

        let watched_pool_service = init_watched_pool_service(&database, listener.clone()).await?;
        info!("watched pool service initialized");

        let dispatcher = SignatureDispatcher::new_default()?;
        info!("dispatcher initialized");

        DispatcherMetrics::register_descriptions();
        IndexerServiceMetrics::register_descriptions();
        info!("Metrics initialized");

        info!("daemon initialized");

        Ok(Self {
            indexer_service,
            watched_pool_service,
            listener,
            dispatcher,
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
            _ = shutdown.cancelled() => tracing::info!("cancellation received — stopping"),
        }
        Ok(())
    }
}

// ── Initialisation helpers ───────────────────────────────────────────────────

/// Connect to the database and apply pending migrations.
async fn init_db(database_url: &str) -> anyhow::Result<Database> {
    let db = Database::connect(database_url).await?;
    tracing::info!("connected to database");
    Ok(db)
}

/// Create the RPC WebSocket listener with its watched protocols.
fn init_listener(config: &Config) -> Arc<RpcListener> {
    let listener = Arc::new(RpcListener::new(
        config.solana_rpc_ws.expose().to_string(),
        config.worker_max_retries,
        config.mode_protocol_centric,
    ));

    listener
}

/// Initialise the indexer service and its repository dependencies.
async fn init_indexer_service(
    config: &Config,
    database: &Database,
) -> anyhow::Result<Arc<IndexerService>> {
    let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.expose().to_string()));
    info!("RPC HTTP client initialized: {}", config.solana_rpc_http);

    let pg_swap_event_repo = Arc::new(PgSwapEventRepository::new(database.pool().clone()));
    let pg_liquidity_event_repo =
        Arc::new(PgLiquidityEventRepository::new(database.pool().clone()));
    let pg_claim_position_fee_repo = Arc::new(PgClaimPositionFeeEventRepository::new(
        database.pool().clone(),
    ));
    let pg_claim_reward_repo = Arc::new(PgClaimRewardEventRepository::new(database.pool().clone()));
    let pg_pool_repo = Arc::new(PgPoolRepository::new(database.pool().clone()));

    Ok(Arc::new(IndexerService::new(
        pg_swap_event_repo,
        pg_liquidity_event_repo,
        pg_claim_position_fee_repo,
        pg_claim_reward_repo,
        pg_pool_repo,
        rpc_client,
    )))
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
