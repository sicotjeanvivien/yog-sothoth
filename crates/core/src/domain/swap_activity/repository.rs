use async_trait::async_trait;
use chrono::TimeDelta;
use solana_pubkey::Pubkey;

use super::{PoolSwapBucket, ProtocolSwapActivity, SwapActivity};
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, TranchePeriod, Window};

/// The swaps' measures. `protocol` narrows a read to one protocol, `None` reads them all;
/// none counts an hour of `period` before its `covered_since`.
#[async_trait]
pub trait SwapActivityLookup: Send + Sync {
    /// One row per protocol with facts in `period`; its pools are those written for `window` at
    /// the period's last hour.
    async fn find_protocol_totals(
        &self,
        period: &FrozenPeriod,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapActivity>>;

    /// The hours of `period` the facts hold, oldest first.
    async fn list_protocol_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>>;

    /// By `width` ending at `period.as_of`, then the `width` before: a bucket the facts do not hold
    /// whole is left out.
    async fn list_protocol_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>>;

    /// One row per protocol measured over the tranches of `period`, 0 for one without any pool;
    /// its pools counted distinct, its pool-hours being its pools × tranches.
    async fn find_protocol_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapActivity>>;

    /// The tranches of `period`, oldest first, one per protocol measured, 0 without any pool.
    async fn list_protocol_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>>;

    /// `None` when the pool did not swap in `period`.
    async fn find_pool_totals(
        &self,
        period: &FrozenPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Option<SwapActivity>>;

    async fn list_pool_hourly(
        &self,
        period: &FrozenPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>>;

    /// The pool's swaps over the tranches of `period`; `None` when it did not swap.
    async fn find_pool_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Option<SwapActivity>>;

    /// The pool's swaps by tranche of `period`, 0 in a tranche without.
    async fn list_pool_tranches(
        &self,
        period: &TranchePeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>>;

    async fn list_pool_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>>;
}
