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
    /// `None` over the last hour: a run started in it has only its first hour.
    pub first_hour_volume_usd: Option<Decimal>,
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

/// A bar's volume split by the rotation runs: `in_runs_usd` is `None` where the runs do not
/// cover the bar whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSplit {
    pub volume_usd: Option<Decimal>,
    pub in_runs_usd: Option<Decimal>,
}

impl RunSplit {
    /// The volume outside the runs, never under 0; `None` as soon as one term is unknown.
    pub fn outside_runs_usd(&self) -> Option<Decimal> {
        Some((self.volume_usd? - self.in_runs_usd?).max(Decimal::ZERO))
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
