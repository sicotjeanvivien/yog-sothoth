//! Translation of `cp-amm::EvtWithdrawIneligibleReward` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtWithdrawIneligibleReward;
use crate::domain::{EventPosition, MeteoraDammV2WithdrawIneligibleRewardEvent};

/// Translate an [`EvtWithdrawIneligibleReward`] into a
/// [`MeteoraDammV2WithdrawIneligibleRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_withdraw_ineligible_reward(
    wire: &EvtWithdrawIneligibleReward,
    event_position: EventPosition,
) -> MeteoraDammV2WithdrawIneligibleRewardEvent {
    MeteoraDammV2WithdrawIneligibleRewardEvent {
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
