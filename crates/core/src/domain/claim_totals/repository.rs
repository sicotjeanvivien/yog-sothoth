use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};

use super::{ClaimKind, ClaimTotals};
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, TranchePeriod};

/// The valued claims. ⚠️ Measured from a later tranche than the swaps: every read takes its
/// period from [`Self::find_measured_since`], not from the facts.
#[async_trait]
pub trait ClaimTotalsLookup: Send + Sync {
    /// The first tranche whose claims `protocol`'s writer valued; `None` before it did.
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>>;

    async fn find_position_fee_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals>;

    async fn find_reward_claim_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals>;

    /// `kind`'s claims over the tranches of `period`, its owners distinct.
    async fn find_totals_in_tranches(
        &self,
        kind: ClaimKind,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<ClaimTotals>;

    /// `kind`'s claims by frozen hour of `period`, from its coverage, 0 in an hour without.
    async fn list_hourly(
        &self,
        kind: ClaimKind,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>>;

    /// The same by `width` ending at the period's `as_of`.
    async fn list_buckets(
        &self,
        kind: ClaimKind,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>>;

    /// The same by tranche of `period`.
    async fn list_tranches(
        &self,
        kind: ClaimKind,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<ClaimTotals>>>;
}
