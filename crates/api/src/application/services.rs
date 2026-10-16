//! Application services, between the HTTP handlers and the `yog-core` repository traits. At
//! the root the cross-protocol ones, which name no protocol; under [`meteora`] the services
//! whose repository, params and result are irreducibly one product's.

mod announcement_service;
mod ingestion_service;
mod meteora;
mod network_service;
mod pool_service;
mod signal_service;
mod stats_service;
mod token_service;

pub(crate) use announcement_service::AnnouncementService;
pub(crate) use ingestion_service::{IngestionAggregate, IngestionService};
pub(crate) use meteora::damm_v2::{
    MeteoraDammV2LiquidityListParams, MeteoraDammV2LiquidityService, MeteoraDammV2SwapListParams,
    MeteoraDammV2SwapService,
};
pub(crate) use network_service::NetworkService;
pub(crate) use pool_service::{PoolCurrentStateView, PoolService, PoolServiceDeps, TOP_POOLS_MAX};
pub(crate) use signal_service::{SignalListParams, SignalService};
pub(crate) use stats_service::{StatsAggregate, StatsService};
pub(crate) use token_service::{TokenAggregate, TokenService};
