use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_core::domain::{
    AnnouncementLookup, EventFreshnessRepository, GlobalAnalyticsRepository, NetworkStatusLookup,
    PoolCatalog, SignalFeed, TokenMetadataLookup, TokenPriceLookup,
};
use yog_persistence::{
    Database, PgAnnouncementRepository, PgEventFreshnessRepository, PgGlobalAnalyticsRepository,
    PgHealthChecker, PgNetworkStatusRepository, PgPoolRepository, PgSignalRepository,
    PgTokenMetadataRepository, PgTokenPriceRepository, PoolSettings,
};

use crate::application::{
    AnnouncementService, IngestionService, NetworkService, STREAM_CHANNEL_CAPACITY, SignalService,
    SignalStreamPoller, StatsService, TokenService, WorkSlots,
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
    let global_analytics_repo: Arc<dyn GlobalAnalyticsRepository> =
        Arc::new(PgGlobalAnalyticsRepository::new(db_pool.clone()));
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
        stats_service: Arc::new(StatsService::new(
            global_analytics_repo,
            pool_repo,
            work_slots.clone(),
        )),
        work_slots,
        token_service: Arc::new(TokenService::new(token_metadata_repo, token_price_repo)),
        announcement_service: Arc::new(AnnouncementService::new(announcement_repo)),
        health_checker: Arc::new(PgHealthChecker::new(db_pool)),
        shutdown,
    };
    Ok((state, signal_poller))
}
