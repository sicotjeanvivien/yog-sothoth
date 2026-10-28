use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_core::application::extraction::ExtractionDispatcher;
use yog_core::domain::{
    AnnouncementLookup, EventFreshnessRepository, NetworkStatusLookup, PoolCatalog,
    ProtocolActivityLookup, SignalFeed, TokenActivityFeed, TokenMetadataLookup, TokenPriceLookup,
};
use yog_persistence::{
    Database, PgAnnouncementRepository, PgClaimTotalsRepository, PgDiscoveredTvlRepository,
    PgEventFreshnessRepository, PgHealthChecker, PgLiquidityChangeEventRepository,
    PgLiquidityMoveRepository, PgMeteoraDammV2PoolPropertiesRepository, PgNetworkStatusRepository,
    PgPoolChangeEventRepository, PgPoolCreationEventRepository, PgPoolCreationsRepository,
    PgPoolRepository, PgPositionChangeEventRepository, PgPositionFeeClaimEventRepository,
    PgProtocolActivityRepository, PgProtocolFeeClaimEventRepository, PgProtocolTvlRepository,
    PgRewardChangeEventRepository, PgRewardClaimEventRepository, PgRotationRunRepository,
    PgSignalRepository, PgSwapActivityRepository, PgSwapEventRepository, PgTokenActivityRepository,
    PgTokenMetadataRepository, PgTokenPriceRepository, PoolSettings,
};

use crate::application::{
    AnnouncementService, ClaimTotalsService, DiscoveredTvlService, IngestionService,
    LiquidityChangeEventService, LiquidityMoveService, NetworkService, PoolChangeEventService,
    PoolCreationEventService, PoolCreationsService, PoolService, PositionChangeEventService,
    PositionFeeClaimEventService, ProtocolCatalogService, ProtocolFeeClaimEventService,
    ProtocolTvlService, RewardChangeEventService, RewardClaimEventService, RotationRunService,
    STREAM_CHANNEL_CAPACITY, SignalService, SignalStreamPoller, SwapActivityService,
    SwapEventService, TokenService, WorkSlots,
};
use crate::bootstrap::Config;
use crate::http::{AppState, SSE_MAX_STREAMS, STATEMENT_TIMEOUT, WORK_SLOT_WAIT, WORK_SLOTS};
use anyhow::Context;

/// The handlers' state, and the poller that feeds it — for [`serve`](crate::bootstrap::serve()).
pub(crate) async fn build_app_state(
    config: Config,
    shutdown: CancellationToken,
) -> anyhow::Result<(AppState, SignalStreamPoller)> {
    let database = Database::connect_with(
        config.database_url.expose(),
        PoolSettings {
            statement_timeout: Some(STATEMENT_TIMEOUT),
            ..PoolSettings::DEFAULT
        },
    )
    .await
    .context("failed to connect to database")?;

    let db_pool = database.pool().clone();

    let pool_repo: Arc<dyn PoolCatalog> = Arc::new(PgPoolRepository::new(db_pool.clone()));
    let network_status_repo: Arc<dyn NetworkStatusLookup> =
        Arc::new(PgNetworkStatusRepository::new(db_pool.clone()));
    let event_freshness_repo: Arc<dyn EventFreshnessRepository> =
        Arc::new(PgEventFreshnessRepository::new(db_pool.clone()));
    let token_metadata_repo: Arc<dyn TokenMetadataLookup> =
        Arc::new(PgTokenMetadataRepository::new(db_pool.clone()));
    let token_price_repo: Arc<dyn TokenPriceLookup> =
        Arc::new(PgTokenPriceRepository::new(db_pool.clone()));
    let signal_repo: Arc<dyn SignalFeed> = Arc::new(PgSignalRepository::new(db_pool.clone()));
    let announcement_repo: Arc<dyn AnnouncementLookup> =
        Arc::new(PgAnnouncementRepository::new(db_pool.clone()));
    let protocol_activity_repo: Arc<dyn ProtocolActivityLookup> =
        Arc::new(PgProtocolActivityRepository::new(db_pool.clone()));
    let token_activity_repo: Arc<dyn TokenActivityFeed> =
        Arc::new(PgTokenActivityRepository::new(db_pool.clone()));
    let swap_activity_service = Arc::new(SwapActivityService::new(
        protocol_activity_repo.clone(),
        Arc::new(PgSwapActivityRepository::new(db_pool.clone())),
        token_activity_repo,
    ));
    let pool_creations_service = Arc::new(PoolCreationsService::new(
        protocol_activity_repo.clone(),
        Arc::new(PgPoolCreationsRepository::new(db_pool.clone())),
    ));
    let liquidity_move_service = Arc::new(LiquidityMoveService::new(
        protocol_activity_repo.clone(),
        Arc::new(PgLiquidityMoveRepository::new(db_pool.clone())),
    ));
    let claim_totals_service = Arc::new(ClaimTotalsService::new(
        protocol_activity_repo.clone(),
        Arc::new(PgClaimTotalsRepository::new(db_pool.clone())),
    ));
    let rotation_run_service = Arc::new(RotationRunService::new(
        protocol_activity_repo.clone(),
        Arc::new(PgRotationRunRepository::new(db_pool.clone())),
    ));
    let protocol_tvl_service = Arc::new(ProtocolTvlService::new(
        protocol_activity_repo.clone(),
        Arc::new(PgProtocolTvlRepository::new(db_pool.clone())),
    ));
    let discovered_tvl_service = Arc::new(DiscoveredTvlService::new(
        protocol_activity_repo,
        Arc::new(PgDiscoveredTvlRepository::new(db_pool.clone())),
    ));
    let swap_event_service = Arc::new(SwapEventService::new(Arc::new(PgSwapEventRepository::new(
        db_pool.clone(),
    ))));
    let liquidity_change_event_service = Arc::new(LiquidityChangeEventService::new(Arc::new(
        PgLiquidityChangeEventRepository::new(db_pool.clone()),
    )));
    let pool_creation_event_service = Arc::new(PoolCreationEventService::new(Arc::new(
        PgPoolCreationEventRepository::new(db_pool.clone()),
    )));
    let position_change_event_service = Arc::new(PositionChangeEventService::new(Arc::new(
        PgPositionChangeEventRepository::new(db_pool.clone()),
    )));
    let reward_change_event_service = Arc::new(RewardChangeEventService::new(Arc::new(
        PgRewardChangeEventRepository::new(db_pool.clone()),
    )));
    let pool_change_event_service = Arc::new(PoolChangeEventService::new(Arc::new(
        PgPoolChangeEventRepository::new(db_pool.clone()),
    )));
    let position_fee_claim_event_service = Arc::new(PositionFeeClaimEventService::new(Arc::new(
        PgPositionFeeClaimEventRepository::new(db_pool.clone()),
    )));
    let protocol_fee_claim_event_service = Arc::new(ProtocolFeeClaimEventService::new(Arc::new(
        PgProtocolFeeClaimEventRepository::new(db_pool.clone()),
    )));
    let reward_claim_event_service = Arc::new(RewardClaimEventService::new(Arc::new(
        PgRewardClaimEventRepository::new(db_pool.clone()),
    )));
    let pool_service = Arc::new(PoolService::new(
        pool_repo.clone(),
        Arc::new(PgMeteoraDammV2PoolPropertiesRepository::new(
            db_pool.clone(),
        )),
    ));

    let signal_service = Arc::new(SignalService::new(
        signal_repo.clone(),
        pool_repo.clone(),
        token_metadata_repo.clone(),
    ));

    let (signal_stream, _) = broadcast::channel(STREAM_CHANNEL_CAPACITY);
    let signal_poller = SignalStreamPoller::new(
        signal_repo.clone(),
        signal_service.clone(),
        signal_stream.clone(),
        config.signal_stream_poll,
    );

    let work_slots = WorkSlots::new(WORK_SLOTS, WORK_SLOT_WAIT);

    let state = AppState {
        network_service: Arc::new(NetworkService::new(network_status_repo)),
        ingestion_service: Arc::new(IngestionService::new(event_freshness_repo)),
        signal_service,
        signal_stream,
        stream_slots: Arc::new(Semaphore::new(SSE_MAX_STREAMS)),
        protocol_catalog_service: Arc::new(ProtocolCatalogService::new(
            pool_repo,
            ExtractionDispatcher::new().implemented_protocols(),
            work_slots.clone(),
        )),
        swap_activity_service,
        pool_creations_service,
        liquidity_move_service,
        claim_totals_service,
        rotation_run_service,
        protocol_tvl_service,
        discovered_tvl_service,
        pool_service,
        swap_event_service,
        liquidity_change_event_service,
        pool_creation_event_service,
        position_change_event_service,
        reward_change_event_service,
        pool_change_event_service,
        position_fee_claim_event_service,
        protocol_fee_claim_event_service,
        reward_claim_event_service,
        work_slots,
        token_service: Arc::new(TokenService::new(token_metadata_repo, token_price_repo)),
        announcement_service: Arc::new(AnnouncementService::new(announcement_repo)),
        health_checker: Arc::new(PgHealthChecker::new(db_pool)),
        shutdown,
    };
    Ok((state, signal_poller))
}
