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
//! - [`EvtInitializeReward`] — admin opens a farming reward slot on a pool
//! - [`EvtFundReward`] — funder deposits rewards and (re)sets the emission rate
//! - [`EvtWithdrawIneligibleReward`] — funder reclaims rewards nobody could earn
//! - [`EvtUpdateRewardDuration`] — admin re-paces a slot's emission window
//! - [`EvtUpdateRewardFunder`] — admin transfers the right to fund a slot
//! - [`EvtWithdrawDeadLiquidityReward`] — funder reclaims dead liquidity's share
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
//! ## Events mirrored without an on-chain fixture
//!
//! Most structs here are validated against a real mainnet transaction saved in
//! `core/tests/fixtures/` — the strongest guarantee, since it proves the borsh
//! mirror decodes bytes the program actually emitted.
//!
//! Some are not: low-frequency admin events for which no transaction has been
//! captured yet. For those the layout comes from the cp-amm source alone, and
//! is guarded two ways instead: a field-mapping test in `translator_tests.rs`,
//! and a layout-pinning test in `events_tests.rs` that asserts the payload size
//! and field offsets. Both catch a *future* drift in our mirror; neither can
//! catch a misreading of the source. Each such struct says so in its docs.

use borsh::{BorshDeserialize, BorshSerialize};
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

/// Discriminator for [`EvtClaimProtocolFee`].
pub fn discriminator_claim_protocol_fee() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimProtocolFee")
}

/// Discriminator for [`EvtInitializeReward`].
pub fn discriminator_initialize_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtInitializeReward")
}

/// Discriminator for [`EvtFundReward`].
pub fn discriminator_fund_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtFundReward")
}

/// Discriminator for [`EvtWithdrawIneligibleReward`].
pub fn discriminator_withdraw_ineligible_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtWithdrawIneligibleReward")
}

/// Discriminator for [`EvtUpdateRewardDuration`].
pub fn discriminator_update_reward_duration() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdateRewardDuration")
}

/// Discriminator for [`EvtUpdateRewardFunder`].
pub fn discriminator_update_reward_funder() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdateRewardFunder")
}

/// Discriminator for [`EvtWithdrawDeadLiquidityReward`].
pub fn discriminator_withdraw_dead_liquidity_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtWithdrawDeadLiquidityReward")
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

/// Discriminator for [`EvtPermanentLockPosition`].
pub fn discriminator_permanent_lock_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtPermanentLockPosition")
}

/// Discriminator for [`EvtInitializePool`].
pub fn discriminator_initialize_pool() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtInitializePool")
}

/// Discriminator for [`EvtSetPoolStatus`].
pub fn discriminator_set_pool_status() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSetPoolStatus")
}

/// Discriminator for [`EvtUpdatePoolFees`].
pub fn discriminator_update_pool_fees() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdatePoolFees")
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

/// Mirror of `cp-amm::EvtClaimProtocolFee`.
///
/// Emitted when the protocol operator withdraws Meteora's accrued **protocol**
/// share of trading fees from a pool (distinct from [`EvtClaimPositionFee`],
/// which is an LP claiming *their position's* fees). `token_a_amount` /
/// `token_b_amount` are the absolute amounts withdrawn in this claim, aligned
/// with the canonical pool ordering.
///
/// This is the `emit_cpi!` variant (`ix_claim_protocol_fee`), the one carried
/// as a self-CPI inner instruction and thus decodable here. cp-amm also has an
/// `EvtClaimProtocolFee2` (`ix_claim_protocol_fee2`) with a different schema
/// (single `token_mint` + `amount` + receiver) emitted via a plain `emit!`
/// *log* — not an event_cpi — so it is **not** captured by this pipeline (and
/// cp-amm itself notes that log "could be truncated. should not rely on this").
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimProtocolFee {
    pub pool: Pubkey,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
}

// ---------------------------------------------------------------------------
// Wire events — rewards / farming (liquidity mining)
// ---------------------------------------------------------------------------
//
// A pool carries up to `NUM_REWARDS` reward slots, addressed by `reward_index`.
// Each slot streams one reward token to in-range LPs at a constant rate over a
// duration window. These are the admin/funder side of the farm; the LP side
// (`EvtClaimReward`) is modelled above.

/// Mirror of `cp-amm::EvtInitializeReward`.
///
/// Emitted when an admin **opens a reward slot** on a pool: it declares which
/// token will be distributed (`reward_mint`), who is allowed to fund it
/// (`funder`), which of the pool's slots is being opened (`reward_index`) and
/// the length of a funding window in seconds (`reward_duration`).
///
/// Opening a slot distributes nothing on its own — the tokens and the emission
/// rate arrive with [`EvtFundReward`], which typically follows in the same
/// transaction.
///
/// `funder` and `creator` are frequently the same wallet, so a fixture cannot
/// discriminate their order — it comes from the cp-amm source.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtInitializeReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub funder: Pubkey,
    pub creator: Pubkey,
    pub reward_index: u8,
    pub reward_duration: u64,
}

/// Mirror of `cp-amm::EvtFundReward`.
///
/// The economic core of the farm: the funder deposits `amount` reward tokens
/// into a slot and the program **recomputes the emission rate** over the slot's
/// configured duration.
///
/// ## Rate scale — Q64.64
///
/// `pre_reward_rate` / `post_reward_rate` are reward base units per second in
/// **Q64.64 fixed point**: divide by `2^64` to read them as a plain rate. On a
/// freshly opened slot this holds exactly:
///
/// ```text
/// post_reward_rate == (amount << 64) / reward_duration
/// ```
///
/// ## Carry-forward
///
/// Funding an already-running slot does not discard what is left of the current
/// window: the program folds the undistributed remainder into the new window, so
/// `post_reward_rate` reflects `amount + leftover`, not `amount` alone. cp-amm
/// exposes this only through the rate pair — there is no explicit
/// `carry_forward` field on the event. The leftover is therefore recoverable as
/// `(post_reward_rate * duration >> 64) - amount`.
///
/// `amount` is what the funder sent; `transfer_fee_excluded_amount_in` is what
/// actually landed in the vault. They differ only for Token-2022 mints charging
/// a transfer fee.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtFundReward {
    pub pool: Pubkey,
    pub funder: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    pub amount: u64,
    pub transfer_fee_excluded_amount_in: u64,
    /// Unix timestamp (seconds) at which the current emission window ends.
    pub reward_duration_end: u64,
    pub pre_reward_rate: u128,
    pub post_reward_rate: u128,
}

/// Mirror of `cp-amm::EvtWithdrawIneligibleReward`.
///
/// Emitted when the funder reclaims reward tokens that **nobody could earn**:
/// rewards that accrued while the pool held no eligible (in-range) liquidity
/// would otherwise stay locked in the vault forever. Withdrawable only after
/// the emission window has ended.
///
/// A high `amount` relative to what was funded means the farm largely missed
/// its target — it emitted into an empty pool.
///
/// Note: cp-amm has a second, structurally identical event,
/// `EvtWithdrawDeadLiquidityReward` (same three fields), covering the reward
/// share of permanently locked liquidity with no owner to claim it. It is a
/// *distinct* event with its own discriminator and is not decoded here — no
/// fixture has been captured for it yet.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtWithdrawIneligibleReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub amount: u64,
}

/// Mirror of `cp-amm::EvtUpdateRewardDuration`.
///
/// Emitted when an admin **re-paces a slot**: the length of a funding window
/// changes, which changes the emission rate every subsequent
/// [`EvtFundReward`] will compute (`rate = amount / duration`). It does not
/// re-rate the *current* window on its own.
///
/// Admin-gated: the signer is either the pool creator or an operator holding
/// the `UpdateRewardDuration` permission.
///
/// Durations are in seconds. Layout taken from the cp-amm source
/// (`ix_update_reward_duration.rs`, single `emit_cpi!` site); no on-chain
/// fixture has been captured for this event — see the module-level note on
/// fixture-less events.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtUpdateRewardDuration {
    pub pool: Pubkey,
    pub reward_index: u8,
    pub old_reward_duration: u64,
    pub new_reward_duration: u64,
}

/// Mirror of `cp-amm::EvtUpdateRewardFunder`.
///
/// Emitted when an admin **transfers the right to fund a slot** from one wallet
/// to another. Moves no tokens and does not touch the emission rate — it only
/// changes who may call `fund_reward` on this `reward_index`.
///
/// Admin-gated: pool creator, or an operator holding the `UpdateRewardFunder`
/// permission. Layout taken from the cp-amm source
/// (`ix_update_reward_funder.rs`, single `emit_cpi!` site); no on-chain fixture
/// captured.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtUpdateRewardFunder {
    pub pool: Pubkey,
    pub reward_index: u8,
    pub old_funder: Pubkey,
    pub new_funder: Pubkey,
}

/// Mirror of `cp-amm::EvtWithdrawDeadLiquidityReward`.
///
/// Emitted when the funder reclaims the reward share that accrued to **dead
/// liquidity** — liquidity permanently locked with no owner left to claim it.
/// Like [`EvtWithdrawIneligibleReward`], it returns tokens that would otherwise
/// sit in the vault forever.
///
/// **Emitted conditionally**: cp-amm wraps the `emit_cpi!` in
/// `if dead_liquidity_reward > 0`, so — unlike `EvtWithdrawIneligibleReward`,
/// which emits even for a zero amount — this event never carries `amount == 0`.
///
/// Byte-identical in shape to [`EvtWithdrawIneligibleReward`] (same three
/// fields, same 72-byte payload); only the discriminator separates them. They
/// stay distinct types because they describe different on-chain facts. Layout
/// taken from the cp-amm source (`ix_withdraw_dead_liquidity_reward.rs`); no
/// on-chain fixture captured.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtWithdrawDeadLiquidityReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub amount: u64,
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

/// Mirror of `cp-amm::EvtPermanentLockPosition`.
///
/// Emitted when an LP permanently locks part of a position's liquidity (no
/// vesting, never unlocks). `lock_liquidity_amount` is the amount locked by
/// this action; `total_permanent_locked_liquidity` is the position's running
/// total after it. Carries no owner field — only pool and position identify
/// it on-chain.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtPermanentLockPosition {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub lock_liquidity_amount: u128,
    pub total_permanent_locked_liquidity: u128,
}

// ---------------------------------------------------------------------------
// Sub-types referenced by EvtInitializePool
// ---------------------------------------------------------------------------

/// Mirror of `cp-amm::BaseFeeParameters`.
///
/// An opaque 27-byte packed blob on the program side (fee scheduler config).
/// We do not interpret it here — it is captured losslessly and decoded later
/// by the dedicated fee-tier work. Reproduced as a fixed array so the borsh
/// layout of the surrounding [`PoolFeeParameters`] stays byte-exact.
#[derive(Debug, Clone, Copy, BorshDeserialize, BorshSerialize)]
pub struct BaseFeeParameters {
    pub data: [u8; 27],
}

/// Mirror of `cp-amm::DynamicFeeParameters`.
#[derive(Debug, Clone, Copy, BorshDeserialize, BorshSerialize)]
pub struct DynamicFeeParameters {
    pub bin_step: u16,
    pub bin_step_u128: u128,
    pub filter_period: u16,
    pub decay_period: u16,
    pub reduction_factor: u16,
    pub max_volatility_accumulator: u32,
    pub variable_fee_control: u32,
}

/// Mirror of `cp-amm::PoolFeeParameters`.
///
/// `dynamic_fee` is borsh-`Option`: a 1-byte tag precedes the inner struct
/// when present. Field order mirrors the on-chain struct exactly — it sits
/// in the middle of [`EvtInitializePool`], so any drift here corrupts every
/// field after it. `BorshSerialize` is derived so the whole blob can be
/// re-serialized and persisted raw (undecoded) under "voie C".
#[derive(Debug, Clone, Copy, BorshDeserialize, BorshSerialize)]
pub struct PoolFeeParameters {
    pub base_fee: BaseFeeParameters,
    pub compounding_fee_bps: u16,
    pub padding: u8,
    pub dynamic_fee: Option<DynamicFeeParameters>,
}

/// Mirror of `cp-amm::EvtInitializePool`.
///
/// Pool genesis: carries both mints, the initial AMM state (sqrt price /
/// bounds, liquidity), the fee configuration, and the seeded token amounts.
/// `pool_fees` is captured but not interpreted (see [`PoolFeeParameters`]).
///
/// Field order mirrors the on-chain struct exactly — do not reorder.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtInitializePool {
    pub pool: Pubkey,
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub creator: Pubkey,
    pub payer: Pubkey,
    pub alpha_vault: Pubkey,
    pub pool_fees: PoolFeeParameters,
    pub sqrt_min_price: u128,
    pub sqrt_max_price: u128,
    pub activation_type: u8,
    pub collect_fee_mode: u8,
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub activation_point: u64,
    pub token_a_flag: u8,
    pub token_b_flag: u8,
    pub token_a_amount: u64,
    pub token_b_amount: u64,
    pub total_amount_a: u64,
    pub total_amount_b: u64,
    pub pool_type: u8,
}

/// Mirror of `cp-amm::EvtSetPoolStatus`.
///
/// Emitted when a pool's status flag is changed (e.g. enabled/disabled).
/// `status` is the raw on-chain status byte — not interpreted here.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtSetPoolStatus {
    pub pool: Pubkey,
    pub status: u8,
}

/// Mirror of `cp-amm::EvtUpdatePoolFees`.
///
/// Emitted when a pool's fee parameters are updated by an operator. The
/// nested `UpdatePoolFeesParameters` is **not** modelled — there is no test
/// fixture to validate its (version-sensitive) layout, and "voie C" defers
/// fee interpretation anyway. Instead, [`BorshDeserialize`] reads the two
/// leading pubkeys and captures the remaining payload bytes verbatim into
/// `params_raw`. This is robust to fee-struct schema changes: a later decode
/// works from these stored bytes.
#[derive(Debug, Clone)]
pub struct EvtUpdatePoolFees {
    pub pool: Pubkey,
    pub operator: Pubkey,
    /// Raw, undecoded bytes of the trailing `UpdatePoolFeesParameters`.
    pub params_raw: Vec<u8>,
}

impl BorshDeserialize for EvtUpdatePoolFees {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let pool = Pubkey::deserialize_reader(reader)?;
        let operator = Pubkey::deserialize_reader(reader)?;
        let mut params_raw = Vec::new();
        reader.read_to_end(&mut params_raw)?;
        Ok(Self {
            pool,
            operator,
            params_raw,
        })
    }
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
