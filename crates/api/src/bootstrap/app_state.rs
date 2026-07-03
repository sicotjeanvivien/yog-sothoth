use std::sync::Arc;

use tokio::sync::broadcast;
use yog_core::domain::{
    EventFreshnessRepository, GlobalAnalyticsRepository, MeteoraDammV2LiquidityEventFeed,
    MeteoraDammV2SwapEventFeed, NetworkStatusLookup, PoolAnalyticsRepository, PoolCatalog,
    PoolCurrentStateLookup, SignalFeed, SignalRecord, TokenMetadataLookup, TokenPriceLookup,
};
use yog_persistence::{
    Database, PgEventFreshnessRepository, PgGlobalAnalyticsRepository, PgHealthChecker,
    PgMeteoraDammV2LiquidityEventRepository, PgMeteoraDammV2SwapEventRepository,
    PgNetworkStatusRepository, PgPoolAnalyticsRepository, PgPoolCurrentStateRepository,
    PgPoolRepository, PgSignalRepository, PgTokenMetadataRepository, PgTokenPriceRepository,
};

use crate::application::{
    MeteoraDammV2LiquidityService, MeteoraDammV2SwapService, NetworkStatusService, PoolService,
    STREAM_CHANNEL_CAPACITY, SignalService, SignalStreamPoller, StatsService, TokenService,
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
    pub(crate) signal_service: Arc<SignalService>,
    /// Live end of the signal feed: SSE handlers `subscribe()` here;
    /// the [`SignalStreamPoller`] spawned by the binary is the producer.
    pub(crate) signal_stream: broadcast::Sender<SignalRecord>,
    pub(crate) stats_service: Arc<StatsService>,
    pub(crate) token_service: Arc<TokenService>,
    /// Infra probe — exposed directly because no application logic
    /// surrounds it. See `yog-persistence/health.rs`.
    pub(crate) health_checker: Arc<PgHealthChecker>,
}

impl AppState {
    /// Build the state and the signal-stream poller that feeds it.
    ///
    /// Returned as a pair: the state goes to the router, the poller to
    /// a `tokio::spawn` in `main` — the binary owns the runtime wiring.
    pub(crate) async fn build(config: Config) -> anyhow::Result<(Self, SignalStreamPoller)> {
        let database = Database::connect(config.database_url.expose())
            .await
            .context("failed to connect to database")?;

        let db_pool = database.pool().clone();

        // ── Repositories ────────────────────────────────────────────────
        let pool_repo: Arc<dyn PoolCatalog> = Arc::new(PgPoolRepository::new(db_pool.clone()));
        let global_analytics_repo: Arc<dyn GlobalAnalyticsRepository> =
            Arc::new(PgGlobalAnalyticsRepository::new(db_pool.clone()));
        let pool_current_state_repo: Arc<dyn PoolCurrentStateLookup> =
            Arc::new(PgPoolCurrentStateRepository::new(db_pool.clone()));
        let swap_event_repo: Arc<dyn MeteoraDammV2SwapEventFeed> =
            Arc::new(PgMeteoraDammV2SwapEventRepository::new(db_pool.clone()));
        let liquidity_event_repo: Arc<dyn MeteoraDammV2LiquidityEventFeed> = Arc::new(
            PgMeteoraDammV2LiquidityEventRepository::new(db_pool.clone()),
        );
        let network_status_repo: Arc<dyn NetworkStatusLookup> =
            Arc::new(PgNetworkStatusRepository::new(db_pool.clone()));
        let event_freshness_repo: Arc<dyn EventFreshnessRepository> =
            Arc::new(PgEventFreshnessRepository::new(db_pool.clone()));
        let token_metadata_repo: Arc<dyn TokenMetadataLookup> =
            Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));
        let token_price_repo: Arc<dyn TokenPriceLookup> =
            Arc::new(PgTokenPriceRepository::new(db_pool.clone()));
        let pool_analytics_repo: Arc<dyn PoolAnalyticsRepository> =
            Arc::new(PgPoolAnalyticsRepository::new(db_pool.clone()));
        let signal_repo: Arc<dyn SignalFeed> = Arc::new(PgSignalRepository::new(db_pool.clone()));

        // ── Signal stream (poller → broadcast → SSE handlers) ──────────
        let (signal_stream, _) = broadcast::channel(STREAM_CHANNEL_CAPACITY);
        let signal_poller = SignalStreamPoller::new(
            signal_repo.clone(),
            signal_stream.clone(),
            config.signal_stream_poll,
        );

        // ── Services ────────────────────────────────────────────────────
        let state = Self {
            pool_service: Arc::new(PoolService::new(
                pool_repo.clone(),
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
            signal_service: Arc::new(SignalService::new(signal_repo)),
            signal_stream,
            stats_service: Arc::new(StatsService::new(global_analytics_repo, pool_repo)),
            token_service: Arc::new(TokenService::new(token_metadata_repo, token_price_repo)),
            health_checker: Arc::new(PgHealthChecker::new(db_pool)),
        };
        Ok((state, signal_poller))
    }
}
