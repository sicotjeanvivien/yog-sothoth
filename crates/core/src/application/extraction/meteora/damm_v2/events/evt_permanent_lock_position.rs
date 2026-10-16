//! Wire mirror of `cp-amm::EvtPermanentLockPosition` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtPermanentLockPosition`].
pub fn discriminator_permanent_lock_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtPermanentLockPosition")
}

/// Mirror of `cp-amm::EvtPermanentLockPosition`: `lock_liquidity_amount` locked
/// by this action, `total_permanent_locked_liquidity` the running total after it.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtPermanentLockPosition {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub lock_liquidity_amount: u128,
    pub total_permanent_locked_liquidity: u128,
}
