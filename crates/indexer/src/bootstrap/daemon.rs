use std::sync::Arc;

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    application::services::{IndexerService, WatchedPoolService},
    config::Config,
    infra::{Database, PgWatchedPoolRepository, RpcListener},
};

/// Meteora DAMM v2 pool used for development and testing.
const TEST_POOL: &str = "CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j";

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
        let database = init_db(config.database_url).await?;

        // HTTP client — used by IndexerService to fetch full transaction data
        let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.clone()));
        info!("RPC HTTP client initialized: {}", config.solana_rpc_http);

        let indexer_service = Arc::new(IndexerService::new(Arc::clone(&rpc_client)));
        info!("indexer service initialized");

        // WebSocket client — receives transaction signatures in real time
        let listener = Arc::new(RpcListener::new(
            config.solana_rpc_ws.clone(),
            config.solana_rpc_http.clone(),
        ));
        info!("RPC listener initialized: {}", config.solana_rpc_ws);

        let pg_watched_pool_repository = Arc::new(PgWatchedPoolRepository::new(database.pool()));
        let watched_pool_service = Arc::new(WatchedPoolService::new(
            listener.clone(),
            pg_watched_pool_repository,
        ));
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
    /// 1. restore pool subscriptions persisted in the database
    /// 2. register the dev test pool (phase 1 — hardcoded)
    /// 3. spawn the WebSocket listener task
    /// 4. wait for a task failure or Ctrl+C, then shut down cleanly
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        // Resubscribe to all pools that were active before the last shutdown
        self.watched_pool_service.restore_subscriptions().await?;
        info!("subscriptions restored");

        // TODO phase 3 — remove once pool management is driven by the Next.js API
        self.listener.watch(TEST_POOL.to_string()).await;

        info!("indexer started — watching test pool");

        let indexer_service = self.indexer_service;
        let listener = self.listener;
        // Retained for phase 3 — LISTEN/NOTIFY loop will use these
        let _watched_pool_service = self.watched_pool_service;
        let _database = self.database;

        // Task 1 — WebSocket loop: receives signatures and dispatches to IndexerService
        let ws_task = tokio::spawn({
            let listener = Arc::clone(&listener);
            let shutdown = shutdown.clone();
            async move {
                listener
                    .run(
                        move |signature| {
                            let service = Arc::clone(&indexer_service);
                            async move {
                                service.handle_signature(signature).await;
                            }
                        },
                        shutdown,
                    )
                    .await
            }
        });

        // Task 2 — LISTEN/NOTIFY loop (phase 3)
        // Will listen for pool_changes notifications from PostgreSQL
        // and call listener.watch() / listener.unwatch() accordingly

        tokio::select! {
            result = ws_task => {
                match result {
                    Ok(Ok(())) => tracing::info!("WebSocket listener stopped"),
                    Ok(Err(e)) => {
                        tracing::error!(error = %e, "WebSocket listener failed");
                        return Err(e); 
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "WebSocket listener panicked");
                        return Err(anyhow::anyhow!("WebSocket listener panicked: {e}"));
                    }
                }
            }
            _ = shutdown.cancelled() => {
                tracing::info!("cancellation received — stopping");
            }
        }
        Ok(())
    }
}

/// Connect to the database and apply pending migrations.
async fn init_db(database_url: String) -> anyhow::Result<Database> {
    let db = Database::connect(&database_url).await?;
    tracing::info!("connected to database");
    db.run_migrations().await?;
    tracing::info!("migrations applied");
    Ok(db)
}
