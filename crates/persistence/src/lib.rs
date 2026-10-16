//! Postgres implementations of the `yog-core` repository traits. ⚠️ The crate owns
//! no pool: each process passes its own, opened under its own least-privilege role.

mod backup;
mod database;
mod error;
mod health;
mod repositories;
mod server_info;

pub use backup::{DumpStream, PgTools, ReadabilityCheck};
pub use database::{Database, PoolSettings};
pub use error::BackupError;
pub use health::{HealthError, PgHealthChecker};
pub use repositories::{
    PgAnalyticRepository, PgAnnouncementRepository, PgEventFreshnessRepository,
    PgGlobalAnalyticsRepository, PgLiquidityFlowRepository, PgMaterializationBacklogRepository,
    PgMeteoraDammV2ClaimPositionFeeEventRepository, PgMeteoraDammV2ClaimProtocolFeeEventRepository,
    PgMeteoraDammV2ClaimRewardEventRepository, PgMeteoraDammV2ClosePositionEventRepository,
    PgMeteoraDammV2CreatePositionEventRepository, PgMeteoraDammV2FundRewardEventRepository,
    PgMeteoraDammV2InitializePoolEventRepository, PgMeteoraDammV2InitializeRewardEventRepository,
    PgMeteoraDammV2LiquidityEventRepository, PgMeteoraDammV2LockPositionEventRepository,
    PgMeteoraDammV2PermanentLockPositionEventRepository, PgMeteoraDammV2PoolPropertiesRepository,
    PgMeteoraDammV2SetPoolStatusEventRepository, PgMeteoraDammV2SplitPositionEventRepository,
    PgMeteoraDammV2SwapEventRepository, PgMeteoraDammV2UpdatePoolFeesEventRepository,
    PgMeteoraDammV2UpdateRewardDurationEventRepository,
    PgMeteoraDammV2UpdateRewardFunderEventRepository,
    PgMeteoraDammV2WithdrawDeadLiquidityRewardEventRepository,
    PgMeteoraDammV2WithdrawIneligibleRewardEventRepository, PgMeteoraDlmmPoolPropertiesRepository,
    PgNetworkStatusRepository, PgPoolAnalyticsRepository, PgPoolCurrentStateRepository,
    PgPoolPriceSnapshotRepository, PgPoolRepository, PgSignalRepository, PgSwapFlowRepository,
    PgTokenMetadataRepository, PgTokenPriceRepository, PgWatchedPoolRepository,
};
pub use server_info::{PgServerInfo, ServerVersions};

/// Re-exported so a consumer types its wiring without depending on sqlx.
pub use sqlx::PgPool;
