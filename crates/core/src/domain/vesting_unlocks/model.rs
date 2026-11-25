//! The liquidity the vesting locks release in the days to come, as of the last reading.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// What one position's schedules release at one instant, valued at the reading's prices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VestingUnlock {
    pub protocol: Protocol,
    pub pool_address: Pubkey,
    /// `None` before the token's metadata is fetched.
    pub token_a_symbol: Option<String>,
    pub token_b_symbol: Option<String>,
    pub position: Pubkey,
    pub unlocks_at: DateTime<Utc>,
    /// In L, the pool's unit of liquidity.
    pub liquidity: u128,
    /// `None` when the pool is not valued.
    pub amount_usd: Option<Decimal>,
}
