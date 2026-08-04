//! Translation of `cp-amm::EvtInitializeReward` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtInitializeReward;
use crate::domain::{EventPosition, MeteoraDammV2InitializeRewardEvent};

/// Translate an [`EvtInitializeReward`] into a [`MeteoraDammV2InitializeRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_initialize_reward(
    wire: &EvtInitializeReward,
    event_position: EventPosition,
) -> MeteoraDammV2InitializeRewardEvent {
    MeteoraDammV2InitializeRewardEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        reward_mint: wire.reward_mint,
        funder: wire.funder,
        creator: wire.creator,
        reward_index: wire.reward_index,
        reward_duration: wire.reward_duration,
    }
}
