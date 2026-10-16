use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// What moved from the first position to the second.
/// ⚠️ Its wire field order differs from [`MeteoraDammV2SplitPositionState`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeteoraDammV2SplitAmounts {
    pub permanent_locked_liquidity: u128,
    pub unlocked_liquidity: u128,
    pub vested_liquidity: u128,
    pub fee_a: u64,
    pub fee_b: u64,
    pub reward_0: u64,
    pub reward_1: u64,
}

/// The state of one position **after** the split, its three liquidity buckets apart.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeteoraDammV2SplitPositionState {
    pub unlocked_liquidity: u128,
    pub permanent_locked_liquidity: u128,
    pub vested_liquidity: u128,
    pub fee_a: u64,
    pub fee_b: u64,
    pub reward_0: u64,
    pub reward_1: u64,
}

/// The requested fractions, each over 1e9; the amounts moved are in
/// [`MeteoraDammV2SplitAmounts`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MeteoraDammV2SplitNumerators {
    pub unlocked_liquidity: u32,
    pub permanent_locked_liquidity: u32,
    pub fee_a: u32,
    pub fee_b: u32,
    pub reward_0: u32,
    pub reward_1: u32,
    pub inner_vesting_liquidity: u32,
}

/// A position moves a fraction of its contents to another position, possibly
/// another wallet's: liquidity changing hands that an NFT transfer would hide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2SplitPositionEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub first_owner: Pubkey,
    pub second_owner: Pubkey,
    pub first_position: Pubkey,
    pub second_position: Pubkey,
    /// Pool sqrt price at the split, to value the moved liquidity.
    pub current_sqrt_price: u128,
    pub amounts: MeteoraDammV2SplitAmounts,
    pub first_position_after: MeteoraDammV2SplitPositionState,
    pub second_position_after: MeteoraDammV2SplitPositionState,
    pub numerators: MeteoraDammV2SplitNumerators,
}
