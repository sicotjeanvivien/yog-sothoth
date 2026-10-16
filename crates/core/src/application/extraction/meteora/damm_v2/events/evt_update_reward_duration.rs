//! Wire mirror of `cp-amm::EvtUpdateRewardDuration` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtUpdateRewardDuration`].
pub fn discriminator_update_reward_duration() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdateRewardDuration")
}

/// Mirror of `cp-amm::EvtUpdateRewardDuration` (seconds): re-paces later fundings,
/// not the current window. Layout from the cp-amm source alone, no fixture.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtUpdateRewardDuration {
    pub pool: Pubkey,
    pub reward_index: u8,
    pub old_reward_duration: u64,
    pub new_reward_duration: u64,
}
