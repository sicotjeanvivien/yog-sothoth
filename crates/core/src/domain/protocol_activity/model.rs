//! What each protocol did over a span, from yog-analytic's tables: its flows,
//! its liquidity, its births and its active pools, then its TVL and its largest moves.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::{Protocol, Window};

/// One protocol over a span. The USD sums skip the unvalued pool-hours, which
/// `pool_hours - pool_hours_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolActivity {
    pub protocol: Protocol,
    pub volume_usd: Option<Decimal>,
    pub fees_usd: Option<Decimal>,
    pub lp_fees_usd: Option<Decimal>,
    pub protocol_fees_usd: Option<Decimal>,
    pub referral_fees_usd: Option<Decimal>,
    pub swap_count: i64,
    /// Distinct over the span; `None` when no cycle wrote it at the span's last hour.
    pub active_pools: Option<i64>,
    pub births: i64,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
    /// `None` when the span starts before the protocol's liquidity was first measured.
    pub liquidity: Option<LiquidityFlows>,
}

/// The liquidity added and removed over a span or a bar. The USD sums skip the moves
/// not valued, which `moves - moves_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityFlows {
    pub added_usd: Decimal,
    pub removed_usd: Decimal,
    pub moves: i64,
    pub moves_valued: i64,
}

/// One protocol over one bar of a series, an hour or 24 hours from `from`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolActivityBar {
    pub protocol: Protocol,
    pub from: DateTime<Utc>,
    pub volume_usd: Option<Decimal>,
    pub fees_usd: Option<Decimal>,
    pub swap_count: i64,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
    /// `None` for a bar that starts before the protocol's liquidity was first measured.
    pub liquidity: Option<LiquidityFlows>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidityMoveKind {
    Add,
    Remove,
}

/// One valued liquidity move, with the identity of its pool's tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidityMove {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub signature: String,
    pub pool_address: String,
    pub token_a_symbol: Option<String>,
    pub token_b_symbol: Option<String>,
    pub owner: String,
    pub kind: LiquidityMoveKind,
    pub amount_usd: Decimal,
}

/// One protocol's TVL as last measured during the hour that closes at `at`: its pools
/// valued by their priced sides; `pools_unvalued` counts those with no side valued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolTvl {
    pub protocol: Protocol,
    pub at: DateTime<Utc>,
    pub tvl_usd: Option<Decimal>,
    pub pools_priced: i64,
    pub pools_unvalued: i64,
}

/// The part of a TVL held by the pools first seen in `window`, their birth unobserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTvl {
    pub window: Window,
    pub tvl_usd: Decimal,
    pub pools: i64,
}

/// The last TVL of a protocol, with its discovered part for each window measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestProtocolTvl {
    pub tvl: ProtocolTvl,
    pub discovered: Vec<DiscoveredTvl>,
}
