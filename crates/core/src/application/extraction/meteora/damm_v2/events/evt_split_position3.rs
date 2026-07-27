//! Wire mirror of `cp-amm::EvtSplitPosition3` and its Anchor discriminator.
//!
//! ⚠️ `SplitAmountInfo2` and `SplitPositionInfo2` carry the SAME seven fields but
//! NOT in the same order: the two leading `u128`s are swapped. cp-amm declares
//! `permanent_locked_liquidity` first in the former, `unlocked_liquidity` first
//! in the latter. Both are `u128`, so a mix-up cannot fail to deserialize — it
//! silently swaps two liquidity figures. Do not "harmonise" these two structs.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtSplitPosition3`].
pub fn discriminator_split_position3() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSplitPosition3")
}

/// Discriminator for the deprecated `EvtSplitPosition2`.
///
/// No mirror struct exists for it on purpose — see [`EvtSplitPosition3`]. We
/// still compute the discriminator so the extractor can recognise the event and
/// drop it *deliberately*, instead of letting it fall through to the
/// "unknown discriminator" bucket on every single split.
pub fn discriminator_split_position2() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSplitPosition2")
}

/// Mirror of `cp-amm::SplitAmountInfo2` — what actually moved from the first
/// position to the second.
///
/// Field order note: `permanent_locked_liquidity` comes **first** here, unlike
/// [`SplitPositionInfo2`]. See the module comment above.
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

/// Mirror of `cp-amm::SplitPositionInfo2` — the state of one position **after**
/// the split.
///
/// Field order note: `unlocked_liquidity` comes **first** here, unlike
/// [`SplitAmountInfo2`]. See the module comment above.
///
/// This is the v3 shape: v2's `SplitPositionInfo` had a single `liquidity`
/// field conflating the three buckets, which is precisely why cp-amm versioned
/// the event.
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

/// Mirror of `cp-amm::SplitPositionParameters3` — the fractions the caller
/// asked for, each a numerator over `SPLIT_POSITION_DENOMINATOR` (1e9).
///
/// Requested fractions, not outcomes: the amounts actually moved are in
/// [`SplitAmountInfo2`]. `inner_vesting_liquidity_numerator` is the v3
/// addition — v2 could not express a split of vesting liquidity.
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

/// Mirror of `cp-amm::EvtSplitPosition3`.
///
/// A position transfers a **fraction of its contents to another position**,
/// possibly owned by a different wallet. Each component is split independently
/// by its own numerator over 1e9: unlocked liquidity, permanently locked
/// liquidity, vesting liquidity, pending fees A/B, and pending farm rewards 0/1.
///
/// Product angle: a split moves liquidity **between two wallets and leaves a
/// traceable event**, unlike transferring the position NFT outright — the blind
/// spot of any LP-concentration score. Splits are therefore visible to
/// concentration analytics.
///
/// ## Why v3, when the instructions are `split_position` / `split_position2`
///
/// cp-amm versions events and instructions independently, and the numbers never
/// line up. There is no `split_position3` instruction and no `EvtSplitPosition`
/// v1: both `split_position` and `split_position2` route to the same handler,
/// which emits **`EvtSplitPosition2` and `EvtSplitPosition3` unconditionally,
/// on every split** (the `#[allow(deprecated)]` block around the v2 emission is
/// an attribute scope, not a condition).
///
/// We mirror **v3 only**. It is a strict superset: v2 conflates the three
/// liquidity buckets into one `liquidity` field, lacks `vested_liquidity` in
/// the amounts, and lacks `inner_vesting_liquidity_numerator` in the
/// parameters. Indexing both would be pure double counting, so the v2
/// discriminator is recognised and dropped (see
/// [`discriminator_split_position2`]).
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
