use async_trait::async_trait;

use super::PositionChangeCounts;
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, TranchePeriod};

/// The position changes yog-analytic counted, by protocol and hour.
#[async_trait]
pub trait PositionChangeCountsLookup: Send + Sync {
    /// `protocol`'s changes over the hours of `period`; `None` when it holds no hour of it.
    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>>;

    /// `protocol`'s changes over the tranches of `period`, counted on the events themselves;
    /// `None` for a protocol whose changes are not counted.
    async fn find_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<PositionChangeCounts>>;

    /// `protocol`'s changes by frozen hour of `period`, from its coverage.
    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>>;

    /// The same by 24 hours ending at the period's `as_of`.
    async fn list_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>>;

    /// The same by tranche of `period`, counted on the events themselves.
    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<PositionChangeCounts>>>;
}
