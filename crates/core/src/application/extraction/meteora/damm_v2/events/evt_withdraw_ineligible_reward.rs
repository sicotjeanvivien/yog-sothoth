//! Wire mirror of `cp-amm::EvtWithdrawIneligibleReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtWithdrawIneligibleReward`].
pub fn discriminator_withdraw_ineligible_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtWithdrawIneligibleReward")
}

/// Mirror of `cp-amm::EvtWithdrawIneligibleReward`.
///
/// Emitted when the funder reclaims reward tokens that **nobody could earn**:
/// rewards that accrued while the pool held no eligible (in-range) liquidity
/// would otherwise stay locked in the vault forever. Withdrawable only after
/// the emission window has ended.
///
/// A high `amount` relative to what was funded means the farm largely missed
/// its target — it emitted into an empty pool.
///
/// Note: cp-amm has a second, structurally identical event,
/// `EvtWithdrawDeadLiquidityReward` (same three fields), covering the reward
/// share of permanently locked liquidity with no owner to claim it. It is a
/// *distinct* event with its own discriminator and is not decoded here — no
/// fixture has been captured for it yet.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtWithdrawIneligibleReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub amount: u64,
}
