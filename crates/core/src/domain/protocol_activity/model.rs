//! What each protocol did over a period, from yog-analytic's tables: its flows,
//! its liquidity, its births and its active pools, then its TVL and its largest moves.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::{DiscoveredTvl, LiquidityFlows, Protocol, ProtocolTvl};

/// One protocol over a period. The USD sums skip the unvalued pool-hours, which
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
    /// Distinct over the period; `None` when no cycle wrote it at the period's last hour.
    pub active_pools: Option<i64>,
    pub births: i64,
    pub pool_hours: i64,
    pub pool_hours_valued: i64,
    /// `None` when the period starts before the protocol's liquidity was first measured.
    pub liquidity: Option<LiquidityFlows>,
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

/// The last TVL of a protocol, with its discovered part for each window measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LatestProtocolTvl {
    pub tvl: ProtocolTvl,
    pub discovered: Vec<DiscoveredTvl>,
}
