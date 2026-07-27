//! Wire mirror of `cp-amm::EvtInitializePool` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtInitializePool`].
pub fn discriminator_initialize_pool() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtInitializePool")
}

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
