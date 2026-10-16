//! Wire mirror of `cp-amm::EvtInitializeReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtInitializeReward`].
pub fn discriminator_initialize_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtInitializeReward")
}

/// Mirror of `cp-amm::EvtInitializeReward`: an admin opens a reward slot; tokens
/// and rate arrive with `EvtFundReward`.
/// ⚠️ The `funder`/`creator` order comes from the source: fixtures often share the wallet.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtInitializeReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub funder: Pubkey,
    pub creator: Pubkey,
    pub reward_index: u8,
    pub reward_duration: u64,
}
