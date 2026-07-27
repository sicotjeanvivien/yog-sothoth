//! Wire mirror of `cp-amm::EvtPermanentLockPosition` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtPermanentLockPosition`].
pub fn discriminator_permanent_lock_position() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtPermanentLockPosition")
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
