//! Wire mirror of `cp-amm::EvtWithdrawDeadLiquidityReward` and its Anchor discriminator.

use super::compute_discriminator;
use crate::application::extraction::DISCRIMINATOR_LEN;
use borsh::BorshDeserialize;
use solana_pubkey::Pubkey;

/// Discriminator for [`EvtWithdrawDeadLiquidityReward`].
pub fn discriminator_withdraw_dead_liquidity_reward() -> [u8; DISCRIMINATOR_LEN] {
    compute_discriminator("EvtWithdrawDeadLiquidityReward")
}

/// Mirror of `cp-amm::EvtWithdrawDeadLiquidityReward`.
///
/// Emitted when the funder reclaims the reward share that accrued to **dead
/// liquidity** — liquidity permanently locked with no owner left to claim it.
/// Like [`EvtWithdrawIneligibleReward`], it returns tokens that would otherwise
/// sit in the vault forever.
///
/// **Emitted conditionally**: cp-amm wraps the `emit_cpi!` in
/// `if dead_liquidity_reward > 0`, so — unlike `EvtWithdrawIneligibleReward`,
/// which emits even for a zero amount — this event never carries `amount == 0`.
///
/// Byte-identical in shape to [`EvtWithdrawIneligibleReward`] (same three
/// fields, same 72-byte payload); only the discriminator separates them. They
/// stay distinct types because they describe different on-chain facts. Layout
/// taken from the cp-amm source (`ix_withdraw_dead_liquidity_reward.rs`); no
/// on-chain fixture captured.
#[derive(Debug, Clone, Copy, BorshDeserialize)]
pub struct EvtWithdrawDeadLiquidityReward {
    pub pool: Pubkey,
    pub reward_mint: Pubkey,
    pub amount: u64,
}
