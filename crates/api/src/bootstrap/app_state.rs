use std::sync::Arc;
use yog_core::domain::{
    LiquidityEventRepository, PoolCurrentStateRepository, PoolRepository, SwapEventRepository,
};
use yog_persistence::{
    Database, PgLiquidityEventRepository, PgPoolCurrentStateRepository, PgPoolRepository,
    PgSwapEventRepository,
};

use crate::bootstrap::Config;
use anyhow::Context;

/// Application-level dependencies shared across HTTP handlers.
///
/// `Clone` is cheap because every field is wrapped in `Arc` — axum
/// requires `Clone + Send + Sync + 'static` for its `State` extractor.
///
/// The DB pool is held inside each repository (via `PgPool`, itself an
/// `Arc` internally), so the `Database` wrapper does not need to live
/// on `AppState` after construction.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool_repository: Arc<dyn PoolRepository>,
    pub(crate) pool_current_state_repository: Arc<dyn PoolCurrentStateRepository>,
    pub(crate) swap_event_repository: Arc<dyn SwapEventRepository>,
    pub(crate) liquidity_event_repository: Arc<dyn LiquidityEventRepository>,
}

impl AppState {
    pub(crate) async fn build(config: Config) -> anyhow::Result<Self> {
        let database = Database::connect(config.database_url.expose())
            .await
            .context("failed to connect to database")?;

        let db_pool = database.pool().clone();

        let pool_repository: Arc<dyn PoolRepository> =
            Arc::new(PgPoolRepository::new(db_pool.clone()));

        let pool_current_state_repository: Arc<dyn PoolCurrentStateRepository> =
            Arc::new(PgPoolCurrentStateRepository::new(db_pool.clone()));

        let swap_event_repository: Arc<dyn SwapEventRepository> =
            Arc::new(PgSwapEventRepository::new(db_pool.clone()));

        let liquidity_event_repository: Arc<dyn LiquidityEventRepository> =
            Arc::new(PgLiquidityEventRepository::new(db_pool));

        Ok(Self {
            pool_repository,
            pool_current_state_repository,
            swap_event_repository,
            liquidity_event_repository,
        })
    }
}
