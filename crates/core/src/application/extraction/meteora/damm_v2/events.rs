//! On-chain wire events emitted by the Meteora DAMM v2 (`cp-amm`) program.
//!
//! Each struct in this module mirrors the exact memory layout of an Anchor
//! event from the cp-amm program. The structs are reproduced locally rather
//! than imported from the cp-amm crate to keep `core` free of Solana program
//! dependencies, and to make the borsh layout an explicit, version-controlled
//! contract on our side.
//!
//! ## Source of truth
//!
//! Mirrors the events defined in
//! [MeteoraAg/cp-amm](https://github.com/MeteoraAg/cp-amm) at
//! `programs/cp-amm/src/event.rs`. If the cp-amm program is upgraded with a
//! schema change, these structs must be updated in lockstep.
//!
//! ## How these events reach us on-chain
//!
//! cp-amm uses Anchor's `emit_cpi!` mechanism: each event is emitted as a
//! self-CPI to the program with the event payload as instruction data,
//! prefixed by Anchor's framework-wide `EVENT_IX_TAG_LE` constant followed
//! by the event-specific 8-byte discriminator. See
//! [`crate::protocols::anchor_event`] for the wire format and the generic
//! decoder.
//!
//! ## Discriminators
//!
//! Anchor prefixes each event with an 8-byte discriminator equal to
//! `sha256("event:<EventName>")[..8]`. The values in this module are
//! computed at runtime from the canonical event names (see
//! [`compute_discriminator`]).
//!
//! ## Scope
//!
//! Only the events Yog-Sothoth indexes today are mirrored here:
//!
//! - [`EvtSwap2`] — swap executed against a pool (also covers legacy `swap`
//!   instructions, which share the same handler and emit the same event)
//! - [`EvtLiquidityChange`] — add or remove liquidity (discriminated by
//!   `change_type`)
//! - [`EvtClaimPositionFee`] — LP claims accumulated trading fees
//! - [`EvtClaimReward`] — LP claims farming rewards
//! - [`EvtCreatePosition`] — LP opens a new (empty) position
//! - [`EvtClosePosition`] — LP closes a position
//! - [`EvtLockPosition`] — LP locks a position under a vesting schedule
//!
//! The remaining position-lifecycle, pool-initialization and admin events
//! are added incrementally, one per change.

use borsh::BorshDeserialize;
use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::application::extraction::DISCRIMINATOR_LEN;

// ---------------------------------------------------------------------------
// Discriminator helpers
// ---------------------------------------------------------------------------

/// Compute the 8-byte Anchor event discriminator for an event named `name`.
///
/// Anchor's convention is `sha256("event:<EventName>")[..8]`. This is the
/// inverse of what the `#[event]` macro generates on the program side.
fn compute_discriminator(name: &str) -> [u8; DISCRIMINATOR_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(format!("event:{name}").as_bytes());
    let full = hasher.finalize();
    let mut out = [0u8; DISCRIMINATOR_LEN];
    out.copy_from_slice(&full[..DISCRIMINATOR_LEN]);
    out
}

/// Discriminator for [`EvtSwap2`].
pub fn discriminator_swap2() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSwap2")
}

/// Discriminator for [`EvtLiquidityChange`].
pub fn discriminator_liquidity_change() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtLiquidityChange")
}

/// Discriminator for [`EvtClaimPositionFee`].
pub fn discriminator_claim_position_fee() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimPositionFee")
}

/// Discriminator for [`EvtClaimReward`].
pub fn discriminator_claim_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimReward")
}

/// Discriminator for [`EvtCreatePosition`].
pub fn discriminator_create_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtCreatePosition")
}

/// Discriminator for [`EvtClosePosition`].
pub fn discriminator_close_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClosePosition")
}

/// Discriminator for [`EvtLockPosition`].
pub fn discriminator_lock_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtLockPosition")
}

// ---------------------------------------------------------------------------
// Sub-types referenced by EvtSwap2
// ---------------------------------------------------------------------------

/// Mirror of `cp-amm::SwapParameters2`.
///
/// The semantics of `amount_0` and `amount_1` depend on `swap_mode`:
/// - `ExactIn` / `PartialFill`: `amount_0 = amount_in`, `amount_1 = minimum_amount_out`
/// - `ExactOut`: `amount_0 = amount_out`, `amount_1 = maximum_amount_in`
///
/// `swap_mode` corresponds to cp-amm's `SwapMode` enum:
/// - `0` = `ExactIn`
/// - `1` = `PartialFill`
/// - `2` = `ExactOut`
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct SwapParameters2 {
    pub amount_0: u64,
    pub amount_1: u64,
    pub swap_mode: u8,
}

/// Mirror of `cp-amm::SwapResult2`.
///
/// Captures every fee component computed by the swap engine. The four fee
/// fields (`claiming_fee`, `protocol_fee`, `compounding_fee`, `referral_fee`)
/// must be summed to obtain the total fee charged on the swap.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct SwapResult2 {
    pub included_fee_input_amount: u64,
    pub excluded_fee_input_amount: u64,
    pub amount_left: u64,
    pub output_amount: u64,
    pub next_sqrt_price: u128,
    pub claiming_fee: u64,
    pub protocol_fee: u64,
    pub compounding_fee: u64,
    pub referral_fee: u64,
}

// ---------------------------------------------------------------------------
// Wire events — Cercle 1
// ---------------------------------------------------------------------------

/// Mirror of `cp-amm::EvtSwap2`.
///
/// Emitted by the cp-amm program for every executed swap, including those
/// initiated through the legacy `swap` instruction — both `swap` and `swap2`
/// share the same handler and emit this event.
///
/// The `reserve_*` fields hold the pool reserves **after** the swap, in the
/// canonical `(token_a, token_b)` ordering defined by the pool — this is
/// the stable convention we want for time-series analytics, regardless of
/// swap direction.
///
/// `trade_direction` reflects the direction the user requested:
/// - `0` (`AtoB`): user provided token A, received token B
/// - `1` (`BtoA`): user provided token B, received token A
///
/// `collect_fee_mode` corresponds to cp-amm's `CollectFeeMode` enum:
/// - `0` = `BothToken`
/// - `1` = `OnlyB`
/// - `2` = `Compounding`
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtSwap2 {
    pub pool: Pubkey,
    pub trade_direction: u8,
    pub collect_fee_mode: u8,
    pub has_referral: bool,
    pub params: SwapParameters2,
    pub swap_result: SwapResult2,
    pub included_transfer_fee_amount_in: u64,
    pub included_transfer_fee_amount_out: u64,
    pub excluded_transfer_fee_amount_out: u64,
    pub current_timestamp: u64,
    pub reserve_a_amount: u64,
    pub reserve_b_amount: u64,
}

/// Mirror of `cp-amm::EvtLiquidityChange`.
///
/// Unified event covering both add and remove liquidity operations. The
/// `change_type` field discriminates:
/// - `0`: liquidity added
/// - `1`: liquidity removed
///
/// `reserve_a_amount` / `reserve_b_amount` are post-change reserves in the
/// canonical pool ordering — same convention as [`EvtSwap2`].
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtLiquidityChange {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub transfer_fee_included_token_a_amount: u64,
    pub transfer_fee_included_token_b_amount: u64,
    pub reserve_a_amount: u64,
    pub reserve_b_amount: u64,
    pub liquidity_delta: u128,
    pub token_a_amount_threshold: u64,
    pub token_b_amount_threshold: u64,
    pub change_type: u8,
}

/// Mirror of `cp-amm::EvtClaimPositionFee`.
///
/// Emitted when an LP claims accumulated trading fees on their position.
/// `fee_a_claimed` / `fee_b_claimed` are absolute amounts in each token —
/// the protocol does not expose a "since-last-claim" delta, only the
/// amount transferred in this specific claim.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimPositionFee {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub fee_a_claimed: u64,
    pub fee_b_claimed: u64,
}

/// Mirror of `cp-amm::EvtClaimReward`.
///
/// Emitted when an LP claims farming rewards distributed by a separate
/// `mint_reward` token. `reward_index` identifies the reward stream within
/// the pool (a pool can have multiple concurrent reward streams).
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimReward {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    pub total_reward: u64,
}

/// Mirror of `cp-amm::EvtCreatePosition`.
///
/// Emitted when an LP opens a new position on a pool. The position is
/// represented on-chain by an NFT (`position_nft_mint`); `position` is the
/// PDA holding the position state. Carries no token amounts — a freshly
/// created position is empty until liquidity is added (see
/// [`EvtLiquidityChange`]).
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtCreatePosition {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub position_nft_mint: Pubkey,
}

/// Mirror of `cp-amm::EvtClosePosition`.
///
/// Emitted when an LP closes a position and the position account is torn
/// down on-chain. Same field shape as [`EvtCreatePosition`]; any remaining
/// liquidity/fees are withdrawn through separate events prior to closing.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClosePosition {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub position_nft_mint: Pubkey,
}

/// Mirror of `cp-amm::EvtLockPosition`.
///
/// Emitted when an LP locks a position under a vesting schedule. The locked
/// liquidity unlocks linearly: `cliff_unlock_liquidity` becomes available at
/// `cliff_point`, then `liquidity_per_period` every `period_frequency` for
/// `number_of_period` periods. `vesting` is the account holding the schedule.
///
/// Field order mirrors the on-chain struct exactly (pool, position, owner,
/// vesting, …) — do not reorder, it is the borsh contract.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtLockPosition {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub vesting: Pubkey,
    pub cliff_point: u64,
    pub period_frequency: u64,
    pub cliff_unlock_liquidity: u128,
    pub liquidity_per_period: u128,
    pub number_of_period: u16,
}

// ---------------------------------------------------------------------------
// Wire event sum type
// ---------------------------------------------------------------------------

/// Heterogeneous collection of DAMM v2 wire events extracted from a single
/// transaction. Each variant wraps the borsh-deserialized payload of one
/// Anchor event emission.
#[derive(Debug, Clone, Copy)]
pub enum DammV2WireEvent {
    Swap2(EvtSwap2),
    LiquidityChange(EvtLiquidityChange),
    ClaimPositionFee(EvtClaimPositionFee),
    ClaimReward(EvtClaimReward),
    CreatePosition(EvtCreatePosition),
    ClosePosition(EvtClosePosition),
    LockPosition(EvtLockPosition),
}

impl DammV2WireEvent {
    /// Pool the event refers to. Useful for routing events to per-pool
    /// downstream processing without matching on the variant.
    pub fn pool(&self) -> Pubkey {
        match self {
            Self::Swap2(e) => e.pool,
            Self::LiquidityChange(e) => e.pool,
            Self::ClaimPositionFee(e) => e.pool,
            Self::ClaimReward(e) => e.pool,
            Self::CreatePosition(e) => e.pool,
            Self::ClosePosition(e) => e.pool,
            Self::LockPosition(e) => e.pool,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Sanity check: discriminators are 8 bytes and stable across runs.
    #[test]
    fn discriminators_are_eight_bytes() {
        assert_eq!(discriminator_swap2().len(), DISCRIMINATOR_LEN);
        assert_eq!(discriminator_liquidity_change().len(), DISCRIMINATOR_LEN);
        assert_eq!(discriminator_claim_position_fee().len(), DISCRIMINATOR_LEN);
        assert_eq!(discriminator_claim_reward().len(), DISCRIMINATOR_LEN);
        assert_eq!(discriminator_create_position().len(), DISCRIMINATOR_LEN);
        assert_eq!(discriminator_close_position().len(), DISCRIMINATOR_LEN);
        assert_eq!(discriminator_lock_position().len(), DISCRIMINATOR_LEN);
    }

    /// Sanity check: each event has a distinct discriminator. If two events
    /// ever collide (extremely unlikely with sha256), our dispatch logic
    /// would silently mis-decode one as the other.
    #[test]
    fn discriminators_are_unique() {
        let all = [
            discriminator_swap2(),
            discriminator_liquidity_change(),
            discriminator_claim_position_fee(),
            discriminator_claim_reward(),
            discriminator_create_position(),
            discriminator_close_position(),
            discriminator_lock_position(),
        ];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "discriminator collision at {i}/{j}");
            }
        }
    }
}
