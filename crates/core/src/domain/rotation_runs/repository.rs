use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};

use super::{
    RotationRun, RotationRunCursor, RotationRunOrder, RotationRunScope, RotationRunTotals, RunSplit,
};
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, TranchePeriod, Window};
use crate::{Page, RepositoryResult};

/// The rotation runs yog-analytic drew. ⚠️ Measured from the first tranche whose rotation was
/// written: every read takes its coverage from [`Self::find_measured_since`].
#[async_trait]
pub trait RotationRunLookup: Send + Sync {
    /// The first tranche whose rotation was measured for `protocol`; `None` before it was.
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>>;

    /// The end of the last tranche written; `None` before the first.
    async fn find_written_through(&self) -> RepositoryResult<Option<DateTime<Utc>>>;

    /// The protocol's runs started in `period`: zero runs is a measure, the period being covered.
    async fn find_totals(
        &self,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<RotationRunTotals>;

    /// `scope`'s runs started in `period`, the period of `window`, in `order`, after `cursor`'s
    /// key; forward only.
    async fn list_runs(
        &self,
        period: &TranchePeriod,
        window: Window,
        scope: RotationRunScope,
        order: RotationRunOrder,
        cursor: Option<RotationRunCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RotationRun>>;

    /// The protocol's volume hour by hour over the frozen `period`, split by its runs in the
    /// valued hours from `runs_since`, the first whole hour they were measured.
    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        runs_since: Option<DateTime<Utc>>,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>>;

    /// The same, by `width` ending at the period's `as_of`, split only when whole from
    /// `runs_since` and valued every hour.
    async fn list_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        runs_since: Option<DateTime<Utc>>,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>>;

    /// The same by tranche of `period`, split in the valued tranches from `runs_since`.
    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        runs_since: Option<DateTime<Utc>>,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>>;
}
