//! Wire mirror of `cp-amm::EvtClaimReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtClaimReward`].
pub fn discriminator_claim_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtClaimReward")
}

/// Mirror of `cp-amm::EvtClaimReward`: an LP claims farming rewards from slot
/// `reward_index`.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtClaimReward {
    pub pool: Pubkey,
    pub position: Pubkey,
    pub owner: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    pub total_reward: u64,
}
