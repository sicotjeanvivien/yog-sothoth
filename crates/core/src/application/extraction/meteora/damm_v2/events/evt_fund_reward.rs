//! Wire mirror of `cp-amm::EvtFundReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtFundReward`].
pub fn discriminator_fund_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtFundReward")
}

/// Mirror of `cp-amm::EvtFundReward`: the funder deposits `amount` and the rate is
/// recomputed, in **Q64.64** base units per second, leftover of a running window
/// included. `transfer_fee_excluded_amount_in` is what landed in the vault.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtFundReward {
    pub pool: Pubkey,
    pub funder: Pubkey,
    pub mint_reward: Pubkey,
    pub reward_index: u8,
    pub amount: u64,
    pub transfer_fee_excluded_amount_in: u64,
    /// Unix timestamp (seconds) at which the current emission window ends.
    pub reward_duration_end: u64,
    pub pre_reward_rate: u128,
    pub post_reward_rate: u128,
}
