//! Wire mirror of `cp-amm::EvtLockPosition` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtLockPosition`].
pub fn discriminator_lock_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtLockPosition")
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
