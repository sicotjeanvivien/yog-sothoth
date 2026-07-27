//! Wire mirror of `cp-amm::EvtUpdateRewardDuration` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtUpdateRewardDuration`].
pub fn discriminator_update_reward_duration() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtUpdateRewardDuration")
}

/// Mirror of `cp-amm::EvtUpdateRewardDuration`.
///
/// Emitted when an admin **re-paces a slot**: the length of a funding window
/// changes, which changes the emission rate every subsequent
/// [`EvtFundReward`] will compute (`rate = amount / duration`). It does not
/// re-rate the *current* window on its own.
///
/// Admin-gated: the signer is either the pool creator or an operator holding
/// the `UpdateRewardDuration` permission.
///
/// Durations are in seconds. Layout taken from the cp-amm source
/// (`ix_update_reward_duration.rs`, single `emit_cpi!` site); no on-chain
/// fixture has been captured for this event — see the module-level note on
/// fixture-less events.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtUpdateRewardDuration {
    pub pool: Pubkey,
    pub reward_index: u8,
    pub old_reward_duration: u64,
    pub new_reward_duration: u64,
}
