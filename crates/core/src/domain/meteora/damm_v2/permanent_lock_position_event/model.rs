use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// LP permanently locking part of a position's liquidity.
///
/// Unlike [`crate::domain::MeteoraDammV2LockPositionEvent`] (vesting, unlocks
/// over time), a permanent lock never unlocks. `lock_liquidity_amount` is the
/// amount locked by this action; `total_permanent_locked_liquidity` is the
/// position's running total afterwards. Both are lossless `u128`
/// (`NUMERIC(39, 0)` at the persistence boundary). No `owner` field — only
/// pool and position identify the event on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2PermanentLockPositionEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub position: Pubkey,
    pub lock_liquidity_amount: u128,
    pub total_permanent_locked_liquidity: u128,
}
