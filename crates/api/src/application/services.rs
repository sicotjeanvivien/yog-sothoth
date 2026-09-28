//! Application services — the choreography layer between the HTTP handlers and
//! the `yog-core` repository traits.
//!
//! Split by *reach*, mirroring [`yog_core::domain`]:
//!
//! - at the root, the **cross-protocol** services. They serve every protocol's
//!   data and name none: `PoolService` reaches a pool's per-protocol properties
//!   through `PoolPropertiesLookup` and learns nothing about which protocol
//!   answered.
//! - under [`meteora`], the **per-protocol** services, one module per product.
//!   A service belongs there when its repository, params and result are
//!   irreducibly that product's.
//!
//! The distinction used to live in file names (`meteora_damm_v2_swap_service.rs`
//! next to `pool_service.rs`), which made it invisible in a directory listing and
//! easy to erode. It is a path now.

mod announcement_service;
mod meteora;
mod network_status_service;
mod pool_service;
mod signal_service;
mod stats_service;
mod token_service;

pub(crate) use announcement_service::AnnouncementService;
pub(crate) use meteora::damm_v2::{
    MeteoraDammV2LiquidityListParams, MeteoraDammV2LiquidityService, MeteoraDammV2SwapListParams,
    MeteoraDammV2SwapService,
};
pub(crate) use network_status_service::{NetworkStatusAggregate, NetworkStatusService};
pub(crate) use pool_service::{PoolCurrentStateView, PoolService, PoolServiceDeps, TOP_POOLS_MAX};
pub(crate) use signal_service::{SignalListParams, SignalService};
pub(crate) use stats_service::{StatsAggregate, StatsService};
pub(crate) use token_service::{TokenAggregate, TokenService};
