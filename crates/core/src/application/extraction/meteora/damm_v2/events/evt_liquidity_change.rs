//! Wire mirror of `cp-amm::EvtLiquidityChange` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtLiquidityChange`].
pub fn discriminator_liquidity_change() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtLiquidityChange")
}

/// Mirror of `cp-amm::EvtLiquidityChange`: `change_type` 0 = add, 1 = remove;
/// `reserve_*` are after the change, in the pool's own A/B order.
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
