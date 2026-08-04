//! Translation of `cp-amm::EvtWithdrawDeadLiquidityReward` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtWithdrawDeadLiquidityReward;
use crate::domain::{EventPosition, MeteoraDammV2WithdrawDeadLiquidityRewardEvent};

/// Translate an [`EvtWithdrawDeadLiquidityReward`] into a
/// [`MeteoraDammV2WithdrawDeadLiquidityRewardEvent`]. Infallible.
pub(super) fn translate_withdraw_dead_liquidity_reward(
    wire: &EvtWithdrawDeadLiquidityReward,
    event_position: EventPosition,
) -> MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
    MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        reward_mint: wire.reward_mint,
        amount: wire.amount,
    }
}
