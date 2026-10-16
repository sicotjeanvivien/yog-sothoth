//! Wire mirror of `cp-amm::EvtWithdrawDeadLiquidityReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtWithdrawDeadLiquidityReward`].
pub fn discriminator_withdraw_dead_liquidity_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtWithdrawDeadLiquidityReward")
}

/// Mirror of `cp-amm::EvtWithdrawDeadLiquidityReward`: the funder reclaims the share of
/// ownerless locked liquidity; never `amount == 0`. Same shape as
/// `EvtWithdrawIneligibleReward`, another fact. Layout from source alone.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtWithdrawDeadLiquidityReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub amount: u64,
}
