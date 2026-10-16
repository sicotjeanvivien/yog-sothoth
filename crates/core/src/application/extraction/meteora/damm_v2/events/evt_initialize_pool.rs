//! Wire mirror of `cp-amm::EvtInitializePool` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtInitializePool`].
pub fn discriminator_initialize_pool() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtInitializePool")
}

/// Mirror of `cp-amm::BaseFeeParameters`: an opaque 27-byte blob, kept as a fixed
/// array so the layout of [`PoolFeeParameters`] stays byte-exact.
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

/// Mirror of `cp-amm::PoolFeeParameters`, re-serialized to be persisted raw.
/// ⚠️ `dynamic_fee` is a borsh `Option` mid-event: any drift here corrupts every
/// field of [`EvtInitializePool`] after it.
#[derive(Debug, Clone, Copy, BorshDeserialize, BorshSerialize)]
pub struct PoolFeeParameters {
    pub base_fee: BaseFeeParameters,
    pub compounding_fee_bps: u16,
    pub padding: u8,
    pub dynamic_fee: Option<DynamicFeeParameters>,
}

/// Mirror of `cp-amm::EvtInitializePool`, the pool genesis.
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
