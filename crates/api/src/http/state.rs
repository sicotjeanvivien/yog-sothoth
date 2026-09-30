//! The state every handler reads through axum's `State` extractor. Built by
//! the binary (`bootstrap::build_app_state`), owned by the HTTP layer: it is
//! what the handlers need, so it is defined where they are.

use std::sync::Arc;

use tokio::sync::{Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use yog_persistence::PgHealthChecker;

use crate::application::{
    AnnouncementService, EnrichedSignal, MeteoraDammV2LiquidityService, MeteoraDammV2SwapService,
    NetworkStatusService, PoolService, SignalService, StatsService, TokenService, WorkSlots,
};

/// Application-level dependencies shared across HTTP handlers.
///
/// The services (`Arc<XxxService>`), plus the few runtime handles the
/// handlers read directly: the signal broadcast, the stream and work slots,
/// the health probe and the stop token. Handlers never access repositories
/// directly — all orchestration lives in the application layer.
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
    /// the [`SignalStreamPoller`](crate::application::SignalStreamPoller)
    /// spawned by the binary is the producer.
    /// Signals travel enriched, once, behind an `Arc`.
    pub(crate) signal_stream: broadcast::Sender<Arc<EnrichedSignal>>,
    /// One permit per open signal stream,
    /// [`SSE_MAX_STREAMS`](super::SSE_MAX_STREAMS) in all.
    pub(crate) stream_slots: Arc<Semaphore>,
    /// The slots of every expensive read — the slow routes' middleware takes
    /// them, and so do the services' cached computations.
    pub(crate) work_slots: WorkSlots,
    pub(crate) stats_service: Arc<StatsService>,
    pub(crate) token_service: Arc<TokenService>,
    pub(crate) announcement_service: Arc<AnnouncementService>,
    /// Infra probe — exposed directly because no application logic
    /// surrounds it. See `yog-persistence/health.rs`.
    pub(crate) health_checker: Arc<PgHealthChecker>,
    /// Cancelled when the process is asked to stop. Only the SSE stream reads
    /// it: every other response ends on its own, and the graceful shutdown
    /// waits for those.
    pub(crate) shutdown: CancellationToken,
}
