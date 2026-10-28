//! The state every handler reads through axum's `State` extractor. Built by
//! the binary ([`build_app_state`](crate::bootstrap::build_app_state)), owned
//! by the HTTP layer: it is what the handlers need, so it is defined where they
//! are.

use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_persistence::PgHealthChecker;

use crate::application::{
    AnnouncementService, ClaimTotalsService, DiscoveredTvlService, EnrichedSignal,
    IngestionService, LiquidityChangeEventService, LiquidityMoveService, NetworkService,
    PoolChangeEventService, PoolCreationEventService, PoolCreationsService, PoolService,
    PositionChangeEventService, PositionFeeClaimEventService, ProtocolCatalogService,
    ProtocolFeeClaimEventService, ProtocolTvlService, RewardChangeEventService,
    RewardClaimEventService, RotationRunService, SignalService, SwapActivityService,
    SwapEventService, TokenService, WorkSlots,
};

/// The services and the runtime handles the router and handlers read. Handlers never reach a
/// repository, except `readyz`, which calls the health probe itself.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) network_service: Arc<NetworkService>,
    pub(crate) ingestion_service: Arc<IngestionService>,
    pub(crate) signal_service: Arc<SignalService>,
    /// Live end of the signal feed, fed by the
    /// [`SignalStreamPoller`](crate::application::SignalStreamPoller); signals travel enriched.
    pub(crate) signal_stream: broadcast::Sender<Arc<EnrichedSignal>>,
    /// One permit per open signal stream,
    /// [`SSE_MAX_STREAMS`](super::SSE_MAX_STREAMS) in all.
    pub(crate) stream_slots: Arc<Semaphore>,
    /// The slots of every expensive read: the slow routes and the cached computations.
    pub(crate) work_slots: WorkSlots,
    pub(crate) protocol_catalog_service: Arc<ProtocolCatalogService>,
    pub(crate) swap_activity_service: Arc<SwapActivityService>,
    pub(crate) pool_creations_service: Arc<PoolCreationsService>,
    pub(crate) liquidity_move_service: Arc<LiquidityMoveService>,
    pub(crate) claim_totals_service: Arc<ClaimTotalsService>,
    pub(crate) rotation_run_service: Arc<RotationRunService>,
    pub(crate) protocol_tvl_service: Arc<ProtocolTvlService>,
    pub(crate) discovered_tvl_service: Arc<DiscoveredTvlService>,
    pub(crate) pool_service: Arc<PoolService>,
    pub(crate) swap_event_service: Arc<SwapEventService>,
    pub(crate) liquidity_change_event_service: Arc<LiquidityChangeEventService>,
    pub(crate) pool_creation_event_service: Arc<PoolCreationEventService>,
    pub(crate) position_change_event_service: Arc<PositionChangeEventService>,
    pub(crate) reward_change_event_service: Arc<RewardChangeEventService>,
    pub(crate) pool_change_event_service: Arc<PoolChangeEventService>,
    pub(crate) position_fee_claim_event_service: Arc<PositionFeeClaimEventService>,
    pub(crate) protocol_fee_claim_event_service: Arc<ProtocolFeeClaimEventService>,
    pub(crate) reward_claim_event_service: Arc<RewardClaimEventService>,
    pub(crate) token_service: Arc<TokenService>,
    pub(crate) announcement_service: Arc<AnnouncementService>,
    /// Infra probe, exposed directly: no application logic surrounds it.
    pub(crate) health_checker: Arc<PgHealthChecker>,
    /// Cancelled on stop. Only the SSE stream reads it: every other response ends on its own.
    pub(crate) shutdown: CancellationToken,
}
