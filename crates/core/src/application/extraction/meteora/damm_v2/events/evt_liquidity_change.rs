//! Wire mirror of `cp-amm::EvtLiquidityChange` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtLiquidityChange`].
pub fn discriminator_liquidity_change() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtLiquidityChange")
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
