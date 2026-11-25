//! The DAMM v2 liquidity locked at a TVL reading, the part of the reading only this protocol has.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

/// The DAMM v2 liquidity locked at a reading, valued at its prices: a pool is valued when a side
/// is and its vesting known, the others counted apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolTvlMeteoraDammV2 {
    pub locked_usd: Decimal,
    pub permanent_usd: Decimal,
    pub vesting_usd: Decimal,
    pub pools_locked: i64,
    pub pools_locked_unvalued: i64,
    /// The first vesting observed: those locked before it are out of reach.
    pub vesting_covered_since: Option<DateTime<Utc>>,
}
