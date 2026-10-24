use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_core::application::extraction::ExtractionDispatcher;
use yog_core::domain::{
    AnnouncementLookup, EventFreshnessRepository, NetworkStatusLookup, PoolCatalog,
    ProtocolActivityLookup, SignalFeed, TokenActivityFeed, TokenMetadataLookup, TokenPriceLookup,
};
use yog_persistence::{
    Database, PgAnnouncementRepository, PgEventFreshnessRepository, PgHealthChecker,
    PgMeasureRepository, PgNetworkStatusRepository, PgPoolRepository, PgProtocolActivityRepository,
    PgSignalRepository, PgTokenActivityRepository, PgTokenMetadataRepository,
    PgTokenPriceRepository, PoolSettings,
};

use crate::application::{
    ActivityService, AnnouncementService, IngestionService, MeasureLenses, MeasureService,
    NetworkService, ProtocolCatalogService, STREAM_CHANNEL_CAPACITY, SignalService,
    SignalStreamPoller, TokenService, WorkSlots,
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
    let measure_repo = Arc::new(PgMeasureRepository::new(db_pool.clone()));
    let measure_service = Arc::new(MeasureService::new(MeasureLenses {
        hours: protocol_activity_repo.clone(),
        swaps: measure_repo.clone(),
        pool_creations: measure_repo.clone(),
        liquidity_moves: measure_repo.clone(),
        tvl: measure_repo.clone(),
        pool_discoveries: measure_repo,
        pools: pool_repo.clone(),
    }));

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
        activity_service: Arc::new(ActivityService::new(
            protocol_activity_repo,
            token_activity_repo,
        )),
        measure_service,
        work_slots,
        token_service: Arc::new(TokenService::new(token_metadata_repo, token_price_repo)),
        announcement_service: Arc::new(AnnouncementService::new(announcement_repo)),
        health_checker: Arc::new(PgHealthChecker::new(db_pool)),
        shutdown,
    };
    Ok((state, signal_poller))
}
