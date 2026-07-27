use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// What actually moved from the first position to the second.
///
/// Field order differs from [`MeteoraDammV2SplitPositionState`] on the wire —
/// see the note there. Modelled as separate types rather than one shared type
/// precisely so the two can never be confused.
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

/// The state of one position **after** the split.
///
/// The three liquidity buckets are reported separately. The deprecated v2 event
/// collapsed them into a single `liquidity` total, which is why cp-amm versioned
/// the event — and why we index v3 only.
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

/// The fractions the caller requested, each a numerator over
/// `SPLIT_POSITION_DENOMINATOR` (1e9).
///
/// Requested, not realised: the amounts actually moved are in
/// [`MeteoraDammV2SplitAmounts`]. Kept alongside them because the gap between
/// the two is itself informative (rounding, or a component that had nothing to
/// give).
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

/// A position transferring **a fraction of its contents to another position**,
/// possibly owned by a different wallet.
///
/// Each component is split independently: unlocked liquidity, permanently
/// locked liquidity, vesting liquidity, pending fees A/B, pending farm rewards
/// 0/1.
///
/// Product angle: a split moves liquidity **between two wallets and leaves a
/// traceable event**, unlike transferring the position NFT outright — the blind
/// spot of any LP-concentration score. Splits are therefore visible to
/// concentration analytics, which is the reason to index this at all.
///
/// Decoded from `EvtSplitPosition3`. cp-amm also emits a deprecated
/// `EvtSplitPosition2` on every split; it is a strict subset and is dropped at
/// extraction to avoid double counting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2SplitPositionEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub first_owner: Pubkey,
    pub second_owner: Pubkey,
    pub first_position: Pubkey,
    pub second_position: Pubkey,
    /// Pool sqrt price at the time of the split — lets the moved liquidity be
    /// valued without joining back to the swap timeline.
    pub current_sqrt_price: u128,
    pub amounts: MeteoraDammV2SplitAmounts,
    pub first_position_after: MeteoraDammV2SplitPositionState,
    pub second_position_after: MeteoraDammV2SplitPositionState,
    pub numerators: MeteoraDammV2SplitNumerators,
}
