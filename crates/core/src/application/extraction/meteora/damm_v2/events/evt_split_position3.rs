//! Wire mirror of `cp-amm::EvtSplitPosition3` and its Anchor discriminator.
//!
//! ⚠️ `SplitAmountInfo2` and `SplitPositionInfo2` order their two leading `u128`s
//! differently, as cp-amm does: "harmonising" them silently swaps two liquidities.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtSplitPosition3`].
pub fn discriminator_split_position3() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSplitPosition3")
}

/// Discriminator for the deprecated `EvtSplitPosition2`, recognised only to be
/// dropped: it has no mirror.
pub fn discriminator_split_position2() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSplitPosition2")
}

/// Mirror of `cp-amm::SplitAmountInfo2`: what moved from the first position to
/// the second. `permanent_locked_liquidity` comes **first**.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct SplitAmountInfo2 {
    pub permanent_locked_liquidity: u128,
    pub unlocked_liquidity: u128,
    pub vested_liquidity: u128,
    pub fee_a: u64,
    pub fee_b: u64,
    pub reward_0: u64,
    pub reward_1: u64,
}

/// Mirror of `cp-amm::SplitPositionInfo2`: one position **after** the split.
/// `unlocked_liquidity` comes **first**.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct SplitPositionInfo2 {
    pub unlocked_liquidity: u128,
    pub permanent_locked_liquidity: u128,
    pub vested_liquidity: u128,
    pub fee_a: u64,
    pub fee_b: u64,
    pub reward_0: u64,
    pub reward_1: u64,
}

/// Mirror of `cp-amm::SplitPositionParameters3`: the requested fractions, each
/// over 1e9; the amounts moved are in [`SplitAmountInfo2`].
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct SplitPositionParameters3 {
    pub unlocked_liquidity_numerator: u32,
    pub permanent_locked_liquidity_numerator: u32,
    pub fee_a_numerator: u32,
    pub fee_b_numerator: u32,
    pub reward_0_numerator: u32,
    pub reward_1_numerator: u32,
    pub inner_vesting_liquidity_numerator: u32,
}

/// Mirror of `cp-amm::EvtSplitPosition3`: a position moves a fraction of its
/// contents to another position, possibly another wallet's.
/// ⚠️ Every split also emits v2, a subset: mirror v3 only, or a split counts twice.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtSplitPosition3 {
    pub pool: Pubkey,
    pub first_owner: Pubkey,
    pub second_owner: Pubkey,
    pub first_position: Pubkey,
    pub second_position: Pubkey,
    pub current_sqrt_price: u128,
    pub amount_splits: SplitAmountInfo2,
    pub first_position_info: SplitPositionInfo2,
    pub second_position_info: SplitPositionInfo2,
    pub split_position_parameters: SplitPositionParameters3,
}
