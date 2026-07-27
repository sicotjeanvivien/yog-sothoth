//! Translation of `cp-amm::EvtWithdrawDeadLiquidityReward` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtWithdrawDeadLiquidityReward;
use crate::domain::MeteoraDammV2WithdrawDeadLiquidityRewardEvent;

/// Translate an [`EvtWithdrawDeadLiquidityReward`] into a
/// [`MeteoraDammV2WithdrawDeadLiquidityRewardEvent`]. Infallible.
pub(super) fn translate_withdraw_dead_liquidity_reward(
    wire: &EvtWithdrawDeadLiquidityReward,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
    MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        reward_mint: wire.reward_mint,
        amount: wire.amount,
    }
}
