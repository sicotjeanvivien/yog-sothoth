use std::sync::Arc;

use yog_core::domain::{
    EventFreshnessRepository, MeteoraDammV2LiquidityEventRepository,
    MeteoraDammV2SwapEventRepository, NetworkStatusRepository, PoolAnalyticsRepository,
    PoolCurrentStateRepository, PoolRepository, TokenMetadataRepository, TokenPriceRepository,
};
use yog_persistence::{
    Database, PgEventFreshnessRepository, PgHealthChecker, PgMeteoraDammV2LiquidityEventRepository,
    PgMeteoraDammV2SwapEventRepository, PgNetworkStatusRepository, PgPoolAnalyticsRepository,
    PgPoolCurrentStateRepository, PgPoolRepository, PgTokenMetadataRepository,
    PgTokenPriceRepository,
};

use crate::application::{
    MeteoraDammV2LiquidityService, MeteoraDammV2SwapService, NetworkStatusService, PoolService,
    TokenService,
};
use crate::bootstrap::Config;
use anyhow::Context;

/// Application-level dependencies shared across HTTP handlers.
///
/// Every field is a service (`Arc<XxxService>`). Handlers never
/// access repositories directly — all orchestration lives in the
/// application layer.
///
/// `Clone` is cheap: `Arc` clones are reference-count bumps.
/// axum requires `Clone + Send + Sync + 'static` on its `State`.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pool_service: Arc<PoolService>,
    pub(crate) swap_service: Arc<MeteoraDammV2SwapService>,
    pub(crate) liquidity_service: Arc<MeteoraDammV2LiquidityService>,
    pub(crate) network_status_service: Arc<NetworkStatusService>,
    pub(crate) token_service: Arc<TokenService>,
    /// Infra probe — exposed directly because no application logic
    /// surrounds it. See `yog-persistence/health.rs`.
    pub(crate) health_checker: Arc<PgHealthChecker>,
}

impl AppState {
    pub(crate) async fn build(config: Config) -> anyhow::Result<Self> {
        let database = Database::connect(config.database_url.expose())
            .await
            .context("failed to connect to database")?;

        let db_pool = database.pool().clone();

        // ── Repositories ────────────────────────────────────────────────
        let pool_repo: Arc<dyn PoolRepository> = Arc::new(PgPoolRepository::new(db_pool.clone()));
        let pool_current_state_repo: Arc<dyn PoolCurrentStateRepository> =
            Arc::new(PgPoolCurrentStateRepository::new(db_pool.clone()));
        let swap_event_repo: Arc<dyn MeteoraDammV2SwapEventRepository> =
            Arc::new(PgMeteoraDammV2SwapEventRepository::new(db_pool.clone()));
        let liquidity_event_repo: Arc<dyn MeteoraDammV2LiquidityEventRepository> = Arc::new(
            PgMeteoraDammV2LiquidityEventRepository::new(db_pool.clone()),
        );
        let network_status_repo: Arc<dyn NetworkStatusRepository> =
            Arc::new(PgNetworkStatusRepository::new(db_pool.clone()));
        let event_freshness_repo: Arc<dyn EventFreshnessRepository> =
            Arc::new(PgEventFreshnessRepository::new(db_pool.clone()));
        let token_metadata_repo: Arc<dyn TokenMetadataRepository> =
            Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));
        let token_price_repo: Arc<dyn TokenPriceRepository> =
            Arc::new(PgTokenPriceRepository::new(db_pool.clone()));
        let pool_analytics_repo: Arc<dyn PoolAnalyticsRepository> =
            Arc::new(PgPoolAnalyticsRepository::new(db_pool.clone()));

        // ── Services ────────────────────────────────────────────────────
        Ok(Self {
            pool_service: Arc::new(PoolService::new(
                pool_repo,
                pool_current_state_repo,
                pool_analytics_repo,
                token_metadata_repo.clone(),
                token_price_repo.clone(),
            )),
            swap_service: Arc::new(MeteoraDammV2SwapService::new(swap_event_repo)),
            liquidity_service: Arc::new(MeteoraDammV2LiquidityService::new(liquidity_event_repo)),
            network_status_service: Arc::new(NetworkStatusService::new(
                network_status_repo,
                event_freshness_repo,
            )),
            token_service: Arc::new(TokenService::new(token_metadata_repo, token_price_repo)),
            health_checker: Arc::new(PgHealthChecker::new(db_pool)),
        })
    }
}
