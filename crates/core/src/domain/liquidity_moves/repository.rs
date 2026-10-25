use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use super::{LiquidityFlows, LiquidityMove, LiquidityMoveCursor, LiquidityMoveOrder};
use crate::domain::{FrozenPeriod, Protocol, ProtocolBucket, Window};
use crate::{Page, RepositoryResult};

/// The valued liquidity moves. ⚠️ Measured from a later hour than the swaps: every read
/// takes its period from [`Self::find_measured_since`], not from the facts.
#[async_trait]
pub trait LiquidityMoveLookup: Send + Sync {
    /// The first hour the protocol's liquidity was measured; `None` before it was.
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>>;

    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<LiquidityFlows>>;

    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>>;

    async fn list_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>>;

    /// A pool's valued moves in `period`, the period of `window`, in `order`, after `cursor`'s key;
    /// forward only.
    async fn list_pool_moves(
        &self,
        period: &FrozenPeriod,
        window: Window,
        pool: Pubkey,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityMove>>;
}
