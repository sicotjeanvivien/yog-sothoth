use async_trait::async_trait;
use chrono::{DateTime, Utc};

use super::{LatestProtocolTvl, LiquidityMove, ProtocolActivity, ProtocolActivityBar, ProtocolTvl};
use crate::RepositoryResult;
use crate::domain::{FrozenHours, FrozenPeriod, Protocol, Window};

/// Read lens on yog-analytic's protocol tables, the api's. Every read takes a
/// [`FrozenPeriod`] built from [`Self::frozen_hours`]; none counts an hour not frozen.
#[async_trait]
pub trait ProtocolActivityLookup: Send + Sync {
    /// `None` before the first freeze.
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>>;

    /// One row per protocol with facts or births in `period`; its active pools
    /// are those written for `window` at the period's last hour.
    async fn activity(
        &self,
        period: &FrozenPeriod,
        window: Window,
    ) -> RepositoryResult<Vec<ProtocolActivity>>;

    /// The hours of `period` the facts hold, oldest first.
    async fn activity_hourly(
        &self,
        period: &FrozenPeriod,
    ) -> RepositoryResult<Vec<ProtocolActivityBar>>;

    /// The 24 hours ending at `period.as_of`, then the 24 before, and so on:
    /// a bar the facts do not hold whole is left out.
    async fn activity_daily(
        &self,
        period: &FrozenPeriod,
    ) -> RepositoryResult<Vec<ProtocolActivityBar>>;

    /// Each protocol's last TVL measured at or before the hour `last`, with the
    /// discovered parts measured with it.
    async fn tvl_latest(&self, last: DateTime<Utc>) -> RepositoryResult<Vec<LatestProtocolTvl>>;

    /// The TVL measured at each hour from `first` through `last`, oldest first.
    async fn tvl_hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> RepositoryResult<Vec<ProtocolTvl>>;

    /// The `limit` valued liquidity moves of `protocol` in `period` worth the most, largest first.
    async fn liquidity_moves(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
        limit: i64,
    ) -> RepositoryResult<Vec<LiquidityMove>>;
}
