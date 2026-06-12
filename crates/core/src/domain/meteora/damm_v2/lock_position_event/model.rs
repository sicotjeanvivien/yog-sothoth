use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// LP locking a position under a vesting schedule.
///
/// Locked liquidity unlocks linearly: `cliff_unlock_liquidity` becomes
/// available at `cliff_point`, then `liquidity_per_period` every
/// `period_frequency` for `number_of_period` periods. `vesting` is the
/// on-chain account holding the schedule.
///
/// `cliff_unlock_liquidity` and `liquidity_per_period` are lossless `u128`
/// (liquidity units, `NUMERIC(39, 0)` at the persistence boundary).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2LockPositionEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub vesting: Pubkey,
    pub cliff_point: u64,
    pub period_frequency: u64,
    pub cliff_unlock_liquidity: u128,
    pub liquidity_per_period: u128,
    pub number_of_period: u16,
}
