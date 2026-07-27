//! Wire mirror of `cp-amm::EvtUpdateRewardFunder` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtUpdateRewardFunder`].
pub fn discriminator_update_reward_funder() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdateRewardFunder")
}

/// Mirror of `cp-amm::EvtUpdateRewardFunder`.
///
/// Emitted when an admin **transfers the right to fund a slot** from one wallet
/// to another. Moves no tokens and does not touch the emission rate — it only
/// changes who may call `fund_reward` on this `reward_index`.
///
/// Admin-gated: pool creator, or an operator holding the `UpdateRewardFunder`
/// permission. Layout taken from the cp-amm source
/// (`ix_update_reward_funder.rs`, single `emit_cpi!` site); no on-chain fixture
/// captured.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtUpdateRewardFunder {
    pub pool: Pubkey,
    pub reward_index: u8,
    pub old_funder: Pubkey,
    pub new_funder: Pubkey,
}
