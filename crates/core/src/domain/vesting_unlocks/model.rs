//! The liquidity the vesting locks release in the days to come, as of the last reading.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, Window};
use crate::tools::Page;

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

/// The order of the list: the soonest first, or the largest first then the soonest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VestingUnlockOrder {
    #[default]
    Time,
    Amount,
}

/// A page of the releases after `as_of`, the instant of the reading they were written at.
#[derive(Debug, Clone)]
pub struct VestingUnlockPage {
    pub as_of: DateTime<Utc>,
    /// The first vesting observed: those locked before it are not listed.
    pub covered_since: Option<DateTime<Utc>>,
    pub page: Page<VestingUnlock>,
}

/// The last release of a page, in the list of `protocol`'s reading at `as_of` over `window`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VestingUnlockCursor {
    pub protocol: Protocol,
    pub as_of: DateTime<Utc>,
    pub window: Window,
    pub order: VestingUnlockOrder,
    /// The amount of the release, read by the amount order only.
    pub amount_usd: Option<Decimal>,
    pub unlocks_at: DateTime<Utc>,
    pub pool_address: Pubkey,
    pub position: Pubkey,
}
