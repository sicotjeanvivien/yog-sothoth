//! Wire mirror of `cp-amm::EvtSwap2` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtSwap2`].
pub fn discriminator_swap2() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSwap2")
}

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
///
/// A legacy `swap` instruction never reaches us as a distinct shape: its
/// two-field `SwapParameters { amount_in, minimum_amount_out }` is widened by
/// cp-amm's entrypoint into `SwapParameters2 { amount_0: amount_in, amount_1:
/// minimum_amount_out, swap_mode: ExactIn }` before the shared handler runs. So
/// `swap_mode == 0` on a legacy swap is the program's own normalisation, not an
/// assumption of ours.
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

/// Mirror of `cp-amm::EvtSwap2`.
///
/// Emitted by the cp-amm program for every executed swap, including those
/// initiated through the legacy `swap` instruction — both `swap` and `swap2`
/// are routed by cp-amm's custom entrypoint to the *same* handler
/// (`p_handle_swap`), the legacy parameters being widened to
/// [`SwapParameters2`] on the way in. There is exactly one emission site and no
/// `EvtSwap` v1 event exists, so this single mirror covers every swap and
/// carries no double-counting risk.
///
/// **This event is not emitted via `emit_cpi!`** — cp-amm hand-rolls the same
/// wire format on its pinocchio fast path. See the module-level section "The
/// swap path does NOT go through `emit_cpi!`" before auditing or changing
/// anything here.
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
