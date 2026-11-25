//! The contract of the lens's read: the order and cursor it takes, the page it returns.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::VestingUnlock;
use crate::domain::{Protocol, Window};
use crate::tools::Page;

/// The order of the list: the soonest first, or the largest first then the soonest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VestingUnlockOrder {
    #[default]
    Time,
    Amount,
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

/// A page of the releases after `as_of`, the instant of the reading they were written at.
#[derive(Debug, Clone)]
pub struct VestingUnlockPage {
    pub as_of: DateTime<Utc>,
    /// The first vesting observed: those locked before it are not listed.
    pub covered_since: Option<DateTime<Utc>>,
    pub page: Page<VestingUnlock>,
}
