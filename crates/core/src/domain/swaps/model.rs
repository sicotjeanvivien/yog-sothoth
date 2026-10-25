//! The swaps yog-analytic measured, read by scope: every protocol, one protocol, or one pool.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::domain::Protocol;

/// The swaps of a scope over a period or a bucket. The USD sums skip the pool-hours not
/// valued, which `pool_hours - pool_hours_valued` counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapActivity {
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
pub struct ProtocolSwapActivity {
    pub protocol: Protocol,
    pub activity: SwapActivity,
    /// Distinct over the window; `None` when no cycle wrote it at the period's last hour.
    pub pools: Option<i64>,
}

/// One pool's swaps over one bucket of a series.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolSwapBucket {
    pub from: DateTime<Utc>,
    pub activity: SwapActivity,
}
