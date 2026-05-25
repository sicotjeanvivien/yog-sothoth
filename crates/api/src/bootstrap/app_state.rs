use std::sync::Arc;
use yog_core::domain::{
    EventFreshnessRepository, LiquidityEventRepository, NetworkStatusRepository,
    PoolAnalyticsRepository, PoolCurrentStateRepository, PoolRepository, SwapEventRepository,
    TokenMetadataRepository, TokenPriceRepository,
};
use yog_persistence::{
    Database, PgEventFreshnessRepository, PgLiquidityEventRepository, PgNetworkStatusRepository,
    PgPoolAnalyticsRepository, PgPoolCurrentStateRepository, PgPoolRepository,
    PgSwapEventRepository, PgTokenMetadataRepository, PgTokenPriceRepository,
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
    pub(crate) network_status_repository: Arc<dyn NetworkStatusRepository>,
    pub(crate) event_freshness_repository: Arc<dyn EventFreshnessRepository>,
    pub(crate) token_metadata_repository: Arc<dyn TokenMetadataRepository>,
    pub(crate) token_price_repository: Arc<dyn TokenPriceRepository>,
    pub(crate) pool_analytics_repository: Arc<dyn PoolAnalyticsRepository>,
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
            Arc::new(PgLiquidityEventRepository::new(db_pool.clone()));

        let network_status_repository: Arc<dyn NetworkStatusRepository> =
            Arc::new(PgNetworkStatusRepository::new(db_pool.clone()));

        let event_freshness_repository: Arc<dyn EventFreshnessRepository> =
            Arc::new(PgEventFreshnessRepository::new(db_pool.clone()));

        let token_metadata_repository: Arc<dyn TokenMetadataRepository> =
            Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));

        let token_price_repository: Arc<dyn TokenPriceRepository> =
            Arc::new(PgTokenPriceRepository::new(db_pool.clone()));

        let pool_analytics_repository: Arc<dyn PoolAnalyticsRepository> =
            Arc::new(PgPoolAnalyticsRepository::new(db_pool));

        Ok(Self {
            pool_repository,
            pool_current_state_repository,
            swap_event_repository,
            liquidity_event_repository,
            network_status_repository,
            event_freshness_repository,
            token_metadata_repository,
            token_price_repository,
            pool_analytics_repository,
        })
    }
}
