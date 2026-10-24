use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use super::{
    ByProtocol, LiquidityMoveCursor, LiquidityMoveOrder, LiquidityMoveSums, PoolCreationCount,
    PoolDiscoveries, PoolSwapBar, ProtocolBar, ProtocolSwapTotals, SwapSums,
};
use crate::domain::{LiquidityMove, Protocol, ProtocolTvl, Span, Window};
use crate::{Page, RepositoryResult};

/// The swaps' measures. `protocol` narrows a read to one protocol, `None` reads them all;
/// none counts an hour of `span` before its `covered_since`.
#[async_trait]
pub trait SwapMeasures: Send + Sync {
    /// One row per protocol with facts in `span`; its pools are those written for `window` at
    /// the span's last hour.
    async fn protocol_totals(
        &self,
        span: &Span,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapTotals>>;

    /// The hours of `span` the facts hold, oldest first.
    async fn protocol_hourly(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<SwapSums>>>;

    /// The 24 hours ending at `span.as_of`, then the 24 before: a bar the facts do not hold
    /// whole is left out.
    async fn protocol_daily(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<SwapSums>>>;

    /// `None` when the pool did not swap in `span`.
    async fn pool_totals(&self, span: &Span, pool: Pubkey) -> RepositoryResult<Option<SwapSums>>;

    async fn pool_hourly(&self, span: &Span, pool: Pubkey) -> RepositoryResult<Vec<PoolSwapBar>>;

    async fn pool_daily(&self, span: &Span, pool: Pubkey) -> RepositoryResult<Vec<PoolSwapBar>>;
}

/// The pools created, by protocol, under the same span rules as [`SwapMeasures`].
#[async_trait]
pub trait PoolCreationMeasures: Send + Sync {
    async fn totals(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ByProtocol<PoolCreationCount>>>;

    async fn hourly(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<PoolCreationCount>>>;

    async fn daily(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<PoolCreationCount>>>;
}

/// The valued liquidity moves. ⚠️ Measured from a later hour than the swaps: every read
/// takes its span from [`Self::measured_since`], not from the facts.
#[async_trait]
pub trait LiquidityMoveMeasures: Send + Sync {
    /// The first hour the protocol's liquidity was measured; `None` before it was.
    async fn measured_since(&self, protocol: Protocol) -> RepositoryResult<Option<DateTime<Utc>>>;

    async fn totals(
        &self,
        span: &Span,
        protocol: Protocol,
    ) -> RepositoryResult<Option<LiquidityMoveSums>>;

    async fn hourly(
        &self,
        span: &Span,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBar<LiquidityMoveSums>>>;

    async fn daily(
        &self,
        span: &Span,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBar<LiquidityMoveSums>>>;

    /// A pool's valued moves in `span`, in `order`, after `cursor`'s key; forward only.
    async fn pool_moves(
        &self,
        span: &Span,
        pool: Pubkey,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityMove>>;
}

/// The TVL, a reading at an instant: each protocol's reading of an hour, never a sum.
#[async_trait]
pub trait TvlReadings: Send + Sync {
    /// Each protocol's last reading at or before the hour `last`.
    async fn latest(
        &self,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>>;

    /// The readings of the hours `first` through `last` that were measured, oldest first.
    async fn hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>>;
}

/// The pools discovered in a window, read with the TVL reading they were measured at.
#[async_trait]
pub trait PoolDiscoveryMeasures: Send + Sync {
    /// Each protocol's discoveries for `window` at its last reading at or before the hour `last`.
    async fn totals(
        &self,
        last: DateTime<Utc>,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<PoolDiscoveries>>;
}
