use async_trait::async_trait;
use solana_pubkey::Pubkey;

use super::{PoolSwapBucket, ProtocolSwapActivity, SwapActivity};
use crate::RepositoryResult;
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, Window};

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

    /// The 24 hours ending at `period.as_of`, then the 24 before: a bucket the facts do not hold
    /// whole is left out.
    async fn list_protocol_daily(
        &self,
        period: &FrozenPeriod,
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

    async fn list_pool_daily(
        &self,
        period: &FrozenPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>>;
}
