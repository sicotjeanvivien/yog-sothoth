//! Wire mirror of `cp-amm::EvtSwap2` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtSwap2`].
pub fn discriminator_swap2() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtSwap2")
}

/// Mirror of `cp-amm::SwapParameters2`. `swap_mode` 0 `ExactIn` and 1 `PartialFill`:
/// `amount_0` in, `amount_1` min out; 2 `ExactOut`: `amount_0` out, `amount_1` max in.
/// cp-amm widens a legacy `swap` to `ExactIn`.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct SwapParameters2 {
    pub amount_0: u64,
    pub amount_1: u64,
    pub swap_mode: u8,
}

/// Mirror of `cp-amm::SwapResult2`; the total fee is the sum of the four fee fields.
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

/// Mirror of `cp-amm::EvtSwap2`, the one event of every swap, legacy `swap` included.
/// `reserve_*` are **after** the swap, in the pool's own A/B order. `trade_direction`:
/// 0 `AtoB`, 1 `BtoA`; `collect_fee_mode`: 0 `BothToken`, 1 `OnlyB`, 2 `Compounding`.
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
