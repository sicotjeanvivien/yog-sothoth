use std::sync::Arc;

use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use tracing::info;

use crate::{
    application::services::{IndexerService, WatchedPoolService},
    config::Config,
    infra::{Database, PgWatchedPoolRepository, RpcListener},
};

pub(crate) struct Daemon {
    indexer_service: Arc<IndexerService>,
    watched_pool_service: Arc<WatchedPoolService>,
    listener: Arc<RpcListener>,
    database: Database,
}

impl Daemon {
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        // Connect database
        let database = init_db(config.database_url).await?;

        // Initialize RPC client (HTTP) for transaction fetching
        let rpc_client = Arc::new(RpcClient::new(config.solana_rpc_http.clone()));

        // Initialize indexer service
        let indexer_service = Arc::new(IndexerService::new(Arc::clone(&rpc_client)));

        // Initialize RPC listener (WebSocket)
        let listener = Arc::new(RpcListener::new(
            config.solana_rpc_ws.clone(),
            config.solana_rpc_http.clone(),
        ));

        let pg_watched_pool_repository = Arc::new(PgWatchedPoolRepository::new(database.pool()));
        let watched_pool_service = Arc::new(WatchedPoolService::new(
            listener.clone(),
            pg_watched_pool_repository,
        ));

        Ok(Self {
            indexer_service,
            watched_pool_service,
            listener,
            database,
        })
    }

    pub(crate) async fn run(self) -> anyhow::Result<()> {
        // Réabonner les pools persistées
        self.watched_pool_service.restore_subscriptions().await?;

        // Watch the test pool
        self.listener
            .watch("CGPxT5d1uf9a8cKVJuZaJAU76t2EfLGbTmRbfvLLZp5j".to_string())
            .await;

        info!("indexer started — watching test pool");

        // Extraire ce que la closure a besoin avant de consommer self
        let indexer_service = self.indexer_service;
        let listener = self.listener;

        listener
            .run(move |signature| {
                let service = Arc::clone(&indexer_service);
                async move {
                    service.handle_signature(signature).await;
                }
            })
            .await;

        Ok(())
    }

    pub(crate) async fn watch(&self, pool_address: String) {
        unimplemented!("")
    }

    pub(crate) async fn unwatch(&self, pool_address: String) {
        unimplemented!("")
    }
}

async fn init_db(database_url: String) -> anyhow::Result<Database> {
    let db = Database::connect(&database_url).await?;
    tracing::info!("connected to database");
    db.run_migrations().await?;
    tracing::info!("migrations applied");

    Ok(db)
}
