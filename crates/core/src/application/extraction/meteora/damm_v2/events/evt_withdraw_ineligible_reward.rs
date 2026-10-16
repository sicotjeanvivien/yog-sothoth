//! Wire mirror of `cp-amm::EvtWithdrawIneligibleReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtWithdrawIneligibleReward`].
pub fn discriminator_withdraw_ineligible_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtWithdrawIneligibleReward")
}

/// Mirror of `cp-amm::EvtWithdrawIneligibleReward`: the funder reclaims rewards
/// that accrued while no liquidity was in range, after the window ends.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtWithdrawIneligibleReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub amount: u64,
}
