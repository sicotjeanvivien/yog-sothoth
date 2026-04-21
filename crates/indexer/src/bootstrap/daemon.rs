use crate::{
    application::services::IndexerService,
    config::Config,
    infra::{
        db::{
            PgLiquidityEventRepository, PgPoolMetricRepository, PgPoolRepository,
            PgSwapEventRepository,
        },
        Database, RpcListener,
    },
    utils::redact::redact_api_key,
};
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::response::transaction::Signature;
use std::sync::Arc;
use tokio::{
    sync::{mpsc, Semaphore},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::info;
use yog_core::domain::Protocol;

/// Top-level process — owns all runtime dependencies and drives the indexer lifecycle.
///
/// Responsibilities:
/// - initialise all dependencies (database, RPC client, services)
/// - register observed protocols at startup
/// - run the WebSocket listener loop and dispatch signatures to IndexerService
/// - handle graceful shutdown on SIGTERM / Ctrl+C
///
/// from the Next.js API without restarting the process.
pub(crate) struct Daemon {
    indexer_service: Arc<IndexerService>,
    listener: Arc<RpcListener>,
    database: Database,
}

impl Daemon {
    /// Build and wire all runtime dependencies.
    /// Fails fast if the database is unreachable or migrations cannot be applied.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let database = init_db(&config.database_url.expose()).await?;
        info!("database initialized");
        let listener = init_listener(&config).await;
        info!("RPC listener initialized: {}", config.solana_rpc_ws);
        let indexer_service = init_indexer_service(&config, &database).await?;
        info!("indexer service initialized");
        info!("daemon initialized");

        Ok(Self {
            indexer_service,
            listener,
            database,
        })
    }

    /// Start the daemon. Consumes `self` — cannot be called twice.
    ///
    /// Sequence:
    /// 1. Spawn the WebSocket listener task and the indexer task
    /// 2. Wait for a task failure or shutdown signal, then stop cleanly
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel(100_000);

        let ws_task = spawn_websocket_task(Arc::clone(&self.listener), tx, shutdown.clone());
        let indexer_task =
            spawn_indexer_task(Arc::clone(&self.indexer_service), rx, shutdown.clone());

        tokio::select! {
            result = ws_task => {
                shutdown.cancel();
                handle_task_result(result, "WebSocket listener")?
            }
            result = indexer_task => {
                shutdown.cancel();
                handle_task_result(result, "indexer")?
            }
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
    let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.expose().to_string()));
    info!("RPC HTTP client initialized: {}", config.solana_rpc_http);

    let pg_swap_event_repo = Arc::new(PgSwapEventRepository::new(database.pool()));
    let pg_pool_repo = Arc::new(PgPoolRepository::new(database.pool()));
    let pg_pool_metric_repo = Arc::new(PgPoolMetricRepository::new(database.pool()));
    let pg_liquidity_event_repo = Arc::new(PgLiquidityEventRepository::new(database.pool()));

    Ok(Arc::new(IndexerService::new(
        pg_liquidity_event_repo,
        pg_pool_repo,
        pg_pool_metric_repo,
        rpc_client,
        pg_swap_event_repo,
    )))
}

/// Create the RPC WebSocket listener.
async fn init_listener(config: &Config) -> Arc<RpcListener> {
    let listener = Arc::new(RpcListener::new(config.solana_rpc_ws.expose().to_string()));
    listener.watch(Protocol::MeteoraDammV2).await;
    listener
}

/// Spawn the WebSocket listener task.
///
/// Receives transaction signatures from the Solana RPC and dispatches
/// them to IndexerService for parsing and persistence.
fn spawn_websocket_task(
    listener: Arc<RpcListener>,
    tx: mpsc::Sender<(Protocol, Signature)>,
    shutdown: CancellationToken,
) -> JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move { listener.run(tx, shutdown).await })
}

const MAX_CONCURRENT_INDEX_TASKS: usize = 15;

fn spawn_indexer_task(
    indexer_service: Arc<IndexerService>,
    mut rx: mpsc::Receiver<(Protocol, Signature)>,
    shutdown: CancellationToken,
) -> JoinHandle<anyhow::Result<()>> {
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_INDEX_TASKS));

    tokio::spawn(async move {
        loop {
            tokio::select! {
                maybe_msg = rx.recv() => {
                    match maybe_msg {
                        Some((protocol, signature)) => {
                            // Acquire a permit before spawning — backpressure s'il
                            // n'y a plus de permit dispo. Cloner l'Arc pour l'async move.
                            let permit = match Arc::clone(&semaphore).acquire_owned().await {
                                Ok(p) => p,
                                Err(_) => {
                                    tracing::warn!("semaphore fermé — arrêt de l'indexer");
                                    break;
                                }
                            };

                            tracing::debug!(
                                queue_depth = rx.len(),
                                permits_available = semaphore.available_permits(),
                                protocol = %protocol.as_str(),
                                %signature,
                                "dispatch signature"
                            );

                            let svc = Arc::clone(&indexer_service);
                            tokio::spawn(async move {
                                match svc.index_transaction(protocol, signature).await {
                                    Ok(()) => tracing::debug!(%signature, "index_transaction ok"),
                                    Err(e) => {
                                            let msg = redact_api_key(&e.to_string());
                                            tracing::error!(error = %msg, %signature, "index_transaction failed");
                                        }
                                }
                                drop(permit); // explicit — libère le permit
                            });
                        }
                        None => {
                            tracing::info!("channel fermé — arrêt de l'indexer");
                            break;
                        }
                    }
                }
                _ = shutdown.cancelled() => break,
            }
        }
        Ok(())
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
            let msg = redact_api_key(&e.to_string());
            tracing::error!(error = %msg, "{task_name} failed");
            Err(e)
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
        let result: Result<anyhow::Result<()>, _> = Ok(Ok(()));
        assert!(handle_task_result(result, "test task").is_ok());
    }

    #[test]
    fn handle_task_result_task_error_returns_err() {
        let result: Result<anyhow::Result<()>, _> = Ok(Err(anyhow::anyhow!("boom")));
        assert!(handle_task_result(result, "test task").is_err());
    }
}
