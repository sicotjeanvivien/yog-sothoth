//! Translation of `cp-amm::EvtUpdateRewardFunder` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtUpdateRewardFunder;
use crate::domain::{EventPosition, MeteoraDammV2UpdateRewardFunderEvent};

/// Translate an [`EvtUpdateRewardFunder`] into a
/// [`MeteoraDammV2UpdateRewardFunderEvent`]. Infallible.
pub(super) fn translate_update_reward_funder(
    wire: &EvtUpdateRewardFunder,
    event_position: EventPosition,
) -> MeteoraDammV2UpdateRewardFunderEvent {
    MeteoraDammV2UpdateRewardFunderEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        reward_index: wire.reward_index,
        old_funder: wire.old_funder,
        new_funder: wire.new_funder,
    }
}
