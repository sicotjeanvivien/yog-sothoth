//! What yog-analytic measured, read by resource and by scope: every protocol, one protocol,
//! or one pool. A row is cut by protocol only where it carries one.

use solana_pubkey::Pubkey;

use crate::domain::{Protocol, Window};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// The swaps of a scope summed over a period or a bar. The USD sums skip the pool-hours not
/// valued, which `pool_hours - pool_hours_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapSums {
    pub swaps: i64,
    pub volume_usd: Option<Decimal>,
    pub fees_usd: Option<Decimal>,
    pub lp_fees_usd: Option<Decimal>,
    pub protocol_fees_usd: Option<Decimal>,
    pub referral_fees_usd: Option<Decimal>,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
}

/// One protocol's swaps over a period, with its pools that swapped in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolSwapTotals {
    pub protocol: Protocol,
    pub sums: SwapSums,
    /// Distinct over the window; `None` when no cycle wrote it at the period's last hour.
    pub pools: Option<i64>,
}

/// One protocol's measure over a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByProtocol<T> {
    pub protocol: Protocol,
    pub value: T,
}

/// One protocol's measure over one bar of a series, an hour or 24 hours from `from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolBar<T> {
    pub protocol: Protocol,
    pub from: DateTime<Utc>,
    pub value: T,
}

/// One pool's swaps over one bar of a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSwapBar {
    pub from: DateTime<Utc>,
    pub sums: SwapSums,
}

/// The pools created over a period or a bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolCreationCount {
    pub pools: i64,
}

/// The liquidity brought by the pools' creation, added and removed; the sums skip the moves
/// not valued, which `moves - moves_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityMoveSums {
    pub created_usd: Decimal,
    pub added_usd: Decimal,
    pub removed_usd: Decimal,
    pub moves: i64,
    pub moves_valued: i64,
}

/// The order of a list of liquidity moves; each breaks its ties the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LiquidityMoveOrder {
    /// Worth the most first.
    #[default]
    Amount,
    /// The most recent first.
    At,
}

/// The key of the last move of a page, and the list it was read from — its pool, window,
/// order and instant: a page of another list, or read after a freeze, would not follow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityMoveCursor {
    pub pool: Pubkey,
    pub window: Window,
    pub as_of: DateTime<Utc>,
    pub order: LiquidityMoveOrder,
    pub amount_usd: Decimal,
    pub at: DateTime<Utc>,
    pub signature: String,
    pub event_index: i32,
}

/// The pools of a protocol first seen in a window without their birth observed, and what they
/// hold at the TVL reading of the hour that closes at `at`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolDiscoveries {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub pools: i64,
    pub tvl_usd: Decimal,
}
