//! Translation of `cp-amm::EvtUpdateRewardDuration` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtUpdateRewardDuration;
use crate::domain::{EventPosition, MeteoraDammV2UpdateRewardDurationEvent};

/// Translate an [`EvtUpdateRewardDuration`] into a
/// [`MeteoraDammV2UpdateRewardDurationEvent`]. Infallible.
pub(super) fn translate_update_reward_duration(
    wire: &EvtUpdateRewardDuration,
    event_position: EventPosition,
) -> MeteoraDammV2UpdateRewardDurationEvent {
    MeteoraDammV2UpdateRewardDurationEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        reward_index: wire.reward_index,
        old_reward_duration: wire.old_reward_duration,
        new_reward_duration: wire.new_reward_duration,
    }
}
