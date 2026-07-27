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
//! `application/extraction/anchor_event.rs` for the wire format and the
//! generic decoder.
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
//! - [`EvtInitializeReward`] — admin opens a farming reward slot on a pool
//! - [`EvtFundReward`] — funder deposits rewards and (re)sets the emission rate
//! - [`EvtWithdrawIneligibleReward`] — funder reclaims rewards nobody could earn
//! - [`EvtUpdateRewardDuration`] — admin re-paces a slot's emission window
//! - [`EvtUpdateRewardFunder`] — admin transfers the right to fund a slot
//! - [`EvtWithdrawDeadLiquidityReward`] — funder reclaims dead liquidity's share
//! - [`EvtSplitPosition3`] — a position's contents are split toward another
//! - [`EvtCreatePosition`] — LP opens a new (empty) position
//! - [`EvtClosePosition`] — LP closes a position
//! - [`EvtLockPosition`] — LP locks a position under a vesting schedule
//! - [`EvtPermanentLockPosition`] — LP permanently locks position liquidity
//! - [`EvtInitializePool`] — pool genesis (mints, initial state, fee config)
//! - [`EvtSetPoolStatus`] — pool status flag change
//! - [`EvtUpdatePoolFees`] — pool fee parameters update (params captured raw)
//!
//! The remaining position-lifecycle, pool-initialization and admin events
//! are added incrementally, one per change.
//!
//! ## Rewards family (farming / liquidity mining)
//!
//! A pool carries up to `NUM_REWARDS` reward slots (2 in cp-amm today),
//! addressed by `reward_index`. Each slot streams one reward token to in-range
//! LPs at a constant rate over a duration window. [`EvtInitializeReward`],
//! [`EvtFundReward`], [`EvtUpdateRewardDuration`], [`EvtUpdateRewardFunder`],
//! [`EvtWithdrawIneligibleReward`] and [`EvtWithdrawDeadLiquidityReward`] are
//! the admin/funder side of that farm; [`EvtClaimReward`] is the LP side.
//!
//! ## Events mirrored without an on-chain fixture
//!
//! Most structs here are validated against a real mainnet transaction saved in
//! `core/tests/fixtures/damm_v2/` — the strongest guarantee, since it proves the borsh
//! mirror decodes bytes the program actually emitted.
//!
//! Some are not: low-frequency admin events for which no transaction has been
//! captured yet. For those the layout comes from the cp-amm source alone, and
//! is guarded two ways instead: a field-mapping test in `translator_tests.rs`,
//! and a layout-pinning test in `events_tests.rs` that asserts the payload size
//! and field offsets. Both catch a *future* drift in our mirror; neither can
//! catch a misreading of the source. Each such struct says so in its docs.
//!
//! ## The swap path does NOT go through `emit_cpi!` — read this before auditing
//!
//! Every event above is emitted by cp-amm through Anchor's `emit_cpi!` macro
//! **except [`EvtSwap2`]**. Auditing the swap path by grepping the cp-amm source
//! for `emit_cpi!(EvtSwap2` returns *nothing*, and reading the `swap` / `swap2`
//! handlers in `lib.rs` shows an empty `Ok(())` body. Both observations are
//! traps: swap is emitted, and we decode it correctly. Here is why.
//!
//! Swap is cp-amm's hot path, so Meteora rewrote it on **pinocchio** (a
//! zero-copy `no_std` Solana SDK) to save compute units, and installed a custom
//! `#[no_mangle] entrypoint` (`programs/cp-amm/src/entrypoint.rs`) that runs
//! *before* Anchor's. That entrypoint matches the leading instruction bytes:
//!
//! - `Swap::DISCRIMINATOR` or `Swap2::DISCRIMINATOR` → handled entirely in
//!   pinocchio by `p_handle_swap`; Anchor is never entered.
//! - `EVENT_IX_TAG_LE` → `p_event_dispatch`, which only validates that the
//!   event authority is a signer and matches the expected PDA.
//! - anything else → falls back to the regular Anchor `entry()`.
//!
//! The `swap` / `swap2` functions in `lib.rs` are therefore deliberate stubs
//! (`_ctx`, `_params`, empty body). They exist only so Anchor still generates
//! the instruction discriminator and the IDL entry; they never execute.
//!
//! Because the pinocchio path has no Anchor `Context`, it cannot call
//! `emit_cpi!`, so cp-amm rebuilds the emission by hand in `p_emit_cpi`
//! (`instructions/swap/ix_p_swap.rs`): it concatenates `EVENT_IX_TAG_LE` with
//! `anchor_lang::Event::data(&EvtSwap2 { .. })` — itself discriminator + borsh
//! body — and self-CPIs to the program with the single event-authority account,
//! `invoke_signed` under the event-authority seeds.
//!
//! That is byte-for-byte what `emit_cpi!` produces: same tag, same
//! discriminator, same payload, same one-account shape, same signer. Our
//! generic decoder (`decode_anchor_event_cpi`, in
//! `application/extraction/anchor_event.rs`) cannot tell the two apart and does not need to — the swap fixtures in
//! `core/tests/fixtures/damm_v2/` are the empirical proof.
//!
//! One consequence worth keeping in mind: for every other event, `emit_cpi!`
//! re-derives the whole wire format from the struct, so a schema change
//! propagates automatically. Here the tag concatenation and the CPI shape are
//! hand-written code that can be edited independently of [`EvtSwap2`] itself.
//! The discriminator is still macro-derived (via `Event::data`), so a rename
//! still propagates — but this is the one emission site where the framework is
//! not doing the work for us.

use sha2::{Digest, Sha256};
use solana_pubkey::Pubkey;

use crate::application::extraction::DISCRIMINATOR_LEN;

mod evt_claim_position_fee;
mod evt_claim_protocol_fee;
mod evt_claim_reward;
mod evt_close_position;
mod evt_create_position;
mod evt_fund_reward;
mod evt_initialize_pool;
mod evt_initialize_reward;
mod evt_liquidity_change;
mod evt_lock_position;
mod evt_permanent_lock_position;
mod evt_set_pool_status;
mod evt_split_position3;
mod evt_swap2;
mod evt_update_pool_fees;
mod evt_update_reward_duration;
mod evt_update_reward_funder;
mod evt_withdraw_dead_liquidity_reward;
mod evt_withdraw_ineligible_reward;

pub use evt_claim_position_fee::{EvtClaimPositionFee, discriminator_claim_position_fee};
pub use evt_claim_protocol_fee::{EvtClaimProtocolFee, discriminator_claim_protocol_fee};
pub use evt_claim_reward::{EvtClaimReward, discriminator_claim_reward};
pub use evt_close_position::{EvtClosePosition, discriminator_close_position};
pub use evt_create_position::{EvtCreatePosition, discriminator_create_position};
pub use evt_fund_reward::{EvtFundReward, discriminator_fund_reward};
pub use evt_initialize_pool::{
    BaseFeeParameters, DynamicFeeParameters, EvtInitializePool, PoolFeeParameters,
    discriminator_initialize_pool,
};
pub use evt_initialize_reward::{EvtInitializeReward, discriminator_initialize_reward};
pub use evt_liquidity_change::{EvtLiquidityChange, discriminator_liquidity_change};
pub use evt_lock_position::{EvtLockPosition, discriminator_lock_position};
pub use evt_permanent_lock_position::{
    EvtPermanentLockPosition, discriminator_permanent_lock_position,
};
pub use evt_set_pool_status::{EvtSetPoolStatus, discriminator_set_pool_status};
pub use evt_split_position3::{
    EvtSplitPosition3, SplitAmountInfo2, SplitPositionInfo2, SplitPositionParameters3,
    discriminator_split_position2, discriminator_split_position3,
};
pub use evt_swap2::{EvtSwap2, SwapParameters2, SwapResult2, discriminator_swap2};
pub use evt_update_pool_fees::{EvtUpdatePoolFees, discriminator_update_pool_fees};
pub use evt_update_reward_duration::{
    EvtUpdateRewardDuration, discriminator_update_reward_duration,
};
pub use evt_update_reward_funder::{EvtUpdateRewardFunder, discriminator_update_reward_funder};
pub use evt_withdraw_dead_liquidity_reward::{
    EvtWithdrawDeadLiquidityReward, discriminator_withdraw_dead_liquidity_reward,
};
pub use evt_withdraw_ineligible_reward::{
    EvtWithdrawIneligibleReward, discriminator_withdraw_ineligible_reward,
};

// ---------------------------------------------------------------------------
// Discriminator helper
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

// ---------------------------------------------------------------------------
// Wire event sum type
// ---------------------------------------------------------------------------

/// Heterogeneous collection of DAMM v2 wire events extracted from a single
/// transaction. Each variant wraps the borsh-deserialized payload of one
/// Anchor event emission.
///
/// Not `Copy`: the boxed `InitializePool` variant precludes it. Events are
/// moved/iterated by reference, never copied, so this costs nothing.
#[derive(Debug, Clone)]
pub enum DammV2WireEvent {
    Swap2(EvtSwap2),
    LiquidityChange(EvtLiquidityChange),
    ClaimPositionFee(EvtClaimPositionFee),
    ClaimReward(EvtClaimReward),
    ClaimProtocolFee(EvtClaimProtocolFee),
    InitializeReward(EvtInitializeReward),
    FundReward(EvtFundReward),
    WithdrawIneligibleReward(EvtWithdrawIneligibleReward),
    UpdateRewardDuration(EvtUpdateRewardDuration),
    UpdateRewardFunder(EvtUpdateRewardFunder),
    WithdrawDeadLiquidityReward(EvtWithdrawDeadLiquidityReward),
    /// Boxed: 444 bytes of payload, far above every other variant.
    SplitPosition3(Box<EvtSplitPosition3>),
    CreatePosition(EvtCreatePosition),
    ClosePosition(EvtClosePosition),
    LockPosition(EvtLockPosition),
    PermanentLockPosition(EvtPermanentLockPosition),
    /// Boxed: the genesis payload dwarfs every other variant (~380 B vs <100 B),
    /// and it is rare — keep the enum (and `Dispatch`) small.
    InitializePool(Box<EvtInitializePool>),
    SetPoolStatus(EvtSetPoolStatus),
    UpdatePoolFees(EvtUpdatePoolFees),
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
            Self::ClaimProtocolFee(e) => e.pool,
            Self::InitializeReward(e) => e.pool,
            Self::FundReward(e) => e.pool,
            Self::WithdrawIneligibleReward(e) => e.pool,
            Self::UpdateRewardDuration(e) => e.pool,
            Self::UpdateRewardFunder(e) => e.pool,
            Self::WithdrawDeadLiquidityReward(e) => e.pool,
            Self::SplitPosition3(e) => e.pool,
            Self::CreatePosition(e) => e.pool,
            Self::ClosePosition(e) => e.pool,
            Self::LockPosition(e) => e.pool,
            Self::PermanentLockPosition(e) => e.pool,
            Self::InitializePool(e) => e.pool,
            Self::SetPoolStatus(e) => e.pool,
            Self::UpdatePoolFees(e) => e.pool,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
