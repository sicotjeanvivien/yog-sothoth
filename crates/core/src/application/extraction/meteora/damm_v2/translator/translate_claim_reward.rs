//! Translation of `cp-amm::EvtClaimReward` into its domain event.

use crate::application::extraction::meteora::damm_v2::events::EvtClaimReward;
use crate::domain::{EventPosition, MeteoraDammV2ClaimRewardEvent};

/// Translate an [`EvtClaimReward`] into a [`MeteoraDammV2ClaimRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_reward(
    wire: &EvtClaimReward,
    event_position: EventPosition,
) -> MeteoraDammV2ClaimRewardEvent {
    MeteoraDammV2ClaimRewardEvent {
        pool_address: wire.pool,
        signature: event_position.signature,
        timestamp: event_position.timestamp,
        slot: event_position.slot,
        transaction_index: event_position.transaction_index,
        event_index: event_position.event_index,
        position: wire.position,
        owner: wire.owner,
        mint_reward: wire.mint_reward,
        reward_index: wire.reward_index,
        total_reward: wire.total_reward,
    }
}
