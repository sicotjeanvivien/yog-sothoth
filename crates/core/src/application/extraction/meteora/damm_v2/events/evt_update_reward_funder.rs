//! Wire mirror of `cp-amm::EvtUpdateRewardFunder` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtUpdateRewardFunder`].
pub fn discriminator_update_reward_funder() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdateRewardFunder")
}

/// Mirror of `cp-amm::EvtUpdateRewardFunder`: who may fund slot `reward_index`.
/// Layout from the cp-amm source alone, no fixture.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtUpdateRewardFunder {
    pub pool: Pubkey,
    pub reward_index: u8,
    pub old_funder: Pubkey,
    pub new_funder: Pubkey,
}
