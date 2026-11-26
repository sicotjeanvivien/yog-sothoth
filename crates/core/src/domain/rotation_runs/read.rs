use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, Window};

/// Which runs a list reads: one protocol's, or one pool's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationRunScope {
    Protocol(Protocol),
    Pool(Pubkey),
}

/// The order of a list of runs; each breaks its ties by pool, then start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RotationRunOrder {
    /// The most volume first.
    #[default]
    Volume,
    /// The latest start first.
    At,
}

/// The key of the last run of a page, and the list it was read from: its scope, window, order
/// and end, fixed at the first page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationRunCursor {
    pub scope: RotationRunScope,
    pub window: Window,
    pub as_of: DateTime<Utc>,
    pub order: RotationRunOrder,
    pub volume_usd: Decimal,
    pub started_at: DateTime<Utc>,
    pub pool_address: Pubkey,
}
