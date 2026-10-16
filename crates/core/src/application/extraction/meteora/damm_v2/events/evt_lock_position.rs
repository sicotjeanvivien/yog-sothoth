//! Wire mirror of `cp-amm::EvtLockPosition` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtLockPosition`].
pub fn discriminator_lock_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtLockPosition")
}

/// Mirror of `cp-amm::EvtLockPosition`: `cliff_unlock_liquidity` at `cliff_point`,
/// then `liquidity_per_period` every `period_frequency`, `number_of_period` times.
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
