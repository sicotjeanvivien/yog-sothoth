use crate::{
    application::services::{IndexerService, WatchedPoolService},
    config::Config,
    infra::{
        db::{PgLiquidityEventRepository, PgPoolMetricRepository, PgSwapEventRepository},
        Database, PgWatchedPoolRepository, RpcListener,
    },
};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Top-level process — owns all runtime dependencies and drives the indexer lifecycle.
///
/// Responsibilities:
/// - initialise all dependencies (database, RPC client, services)
/// - restore active pool subscriptions from the database on startup
/// - run the WebSocket listener loop and dispatch signatures to IndexerService
/// - handle graceful shutdown on SIGTERM / Ctrl+C
///
/// Phase 3 will add a LISTEN/NOTIFY loop to pick up pool changes
/// from the Next.js API without restarting the process.
pub(crate) struct Daemon {
    indexer_service: Arc<IndexerService>,
    watched_pool_service: Arc<WatchedPoolService>,
    listener: Arc<RpcListener>,
    database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    /// Fails fast if the database is unreachable or migrations cannot be applied.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let database = init_db(&config.database_url).await?;
        info!("database initialized");
        let indexer_service = init_indexer_service(&config, &database).await?;
        info!("indexer service initialized");
        let listener = init_listener(&config).await?;
        info!("RPC listener initialized: {}", config.solana_rpc_ws);
        let watched_pool_service = init_watched_pool_service(&database, listener.clone()).await?;
        info!("watched pool service initialized");
        info!("daemon initialized");

        Ok(Self {
            indexer_service,
            watched_pool_service,
            listener,
            database,
        })
    }

    /// Start the daemon. Consumes `self` — cannot be called twice.
    ///
    /// Sequence:
    /// 1. Restore pool subscriptions persisted in the database
    /// 2. Spawn the WebSocket listener task
    /// 3. Wait for a task failure or shutdown signal, then stop cleanly
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        self.watched_pool_service.restore_subscriptions().await?;
        info!("subscriptions restored");

        let ws_task = spawn_websocket_task(
            Arc::clone(&self.listener),
            Arc::clone(&self.indexer_service),
            shutdown.clone(),
        );

        // Phase 3 — spawn_pool_watcher will be added here.
        // It will receive LISTEN/NOTIFY notifications from PostgreSQL
        // and call WatchedPoolService::watch() / unwatch() accordingly.
        let _database = self.database;

        tokio::select! {
            result = ws_task => handle_task_result(result, "WebSocket listener")?,
            _ = shutdown.cancelled() => tracing::info!("cancellation received — stopping"),
        }
        Ok(())
    }
}

/// Connect to the database and apply pending migrations.
async fn init_db(database_url: &str) -> anyhow::Result<Database> {
    let db = Database::connect(database_url).await?;
    tracing::info!("connected to database");
    db.run_migrations().await?;
    tracing::info!("migrations applied");
    Ok(db)
}

/// Initialise the IndexerService and its repository dependencies.
async fn init_indexer_service(
    config: &Config,
    database: &Database,
) -> anyhow::Result<Arc<IndexerService>> {
    let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.clone()));
    info!("RPC HTTP client initialized: {}", config.solana_rpc_http);

    let pg_swap_event_repo = Arc::new(PgSwapEventRepository::new(database.pool()));
    let pg_pool_metric_repo = Arc::new(PgPoolMetricRepository::new(database.pool()));
    let pg_liquidity_event_repo = Arc::new(PgLiquidityEventRepository::new(database.pool()));

    Ok(Arc::new(IndexerService::new(
        pg_liquidity_event_repo,
        pg_pool_metric_repo,
        rpc_client,
        pg_swap_event_repo,
    )))
}

/// Create the RPC WebSocket listener.
async fn init_listener(config: &Config) -> anyhow::Result<Arc<RpcListener>> {
    Ok(Arc::new(RpcListener::new(
        config.solana_rpc_ws.clone(),
        config.solana_rpc_http.clone(),
    )))
}

/// Initialise the WatchedPoolService and its repository dependency.
async fn init_watched_pool_service(
    database: &Database,
    listener: Arc<RpcListener>,
) -> anyhow::Result<Arc<WatchedPoolService>> {
    let pg_watched_pool_repository = Arc::new(PgWatchedPoolRepository::new(database.pool()));
    Ok(Arc::new(WatchedPoolService::new(
        listener,
        pg_watched_pool_repository,
    )))
}

/// Spawn the WebSocket listener task.
///
/// Receives transaction signatures from the Solana RPC and dispatches
/// them to IndexerService for parsing and persistence.
fn spawn_websocket_task(
    listener: Arc<RpcListener>,
    indexer_service: Arc<IndexerService>,
    shutdown: CancellationToken,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        listener
            .run(
                move |signature| {
                    let service = Arc::clone(&indexer_service);
                    async move { service.handle_signature(signature).await }
                },
                shutdown,
            )
            .await
    })
}

/// Normalise the result of a spawned task into a loggable anyhow::Result.
///
/// Distinguishes three cases: clean stop, task error, and task panic.
fn handle_task_result(
    result: Result<anyhow::Result<()>, tokio::task::JoinError>,
    task_name: &str,
) -> anyhow::Result<()> {
    match result {
        Ok(Ok(())) => {
            tracing::info!("{task_name} stopped");
            Ok(())
        }
        Ok(Err(e)) => {
            tracing::error!(error = %e, "{task_name} failed");
            Err(e)
        }
        Err(e) => {
            tracing::error!(error = %e, "{task_name} panicked");
            Err(anyhow::anyhow!("{task_name} panicked: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_task_result_clean_stop_returns_ok() {
        let result: Result<anyhow::Result<()>, _> = Ok(Ok(()));
        assert!(handle_task_result(result, "test task").is_ok());
    }

    #[test]
    fn handle_task_result_task_error_returns_err() {
        let result: Result<anyhow::Result<()>, _> = Ok(Err(anyhow::anyhow!("boom")));
        assert!(handle_task_result(result, "test task").is_err());
    }
}
