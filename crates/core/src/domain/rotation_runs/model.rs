use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

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
