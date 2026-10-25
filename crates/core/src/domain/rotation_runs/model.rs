//! The rotation runs: each one, and what they sum to over a period.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::{Protocol, Window};

/// A pool's tranches whose quoted reserve turned over, valued on the hourly facts' base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationRun {
    pub protocol: Protocol,
    pub pool_address: Pubkey,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub volume_usd: Decimal,
    pub first_hour_volume_usd: Decimal,
    /// No marked tranche followed within the gap: the run will not grow.
    pub extinct: bool,
    /// The symbols of its pool's tokens, `None` before their metadata is fetched.
    pub token_a_symbol: Option<String>,
    pub token_b_symbol: Option<String>,
}

/// The runs that started in a period, and their volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationRunTotals {
    pub runs: i64,
    /// The distinct pools the runs are of.
    pub pools: i64,
    /// The runs not extinct yet.
    pub in_progress: i64,
    /// From start to end, `None` without a run.
    pub median_duration_seconds: Option<i64>,
    pub p90_duration_seconds: Option<i64>,
    pub volume_usd: Decimal,
    pub first_hour_volume_usd: Decimal,
}

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
