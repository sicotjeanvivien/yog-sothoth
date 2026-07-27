//! Translation of `cp-amm::EvtUpdateRewardDuration` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtUpdateRewardDuration;
use crate::domain::MeteoraDammV2UpdateRewardDurationEvent;

/// Translate an [`EvtUpdateRewardDuration`] into a
/// [`MeteoraDammV2UpdateRewardDurationEvent`]. Infallible.
pub(super) fn translate_update_reward_duration(
    wire: &EvtUpdateRewardDuration,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2UpdateRewardDurationEvent {
    MeteoraDammV2UpdateRewardDurationEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        reward_index: wire.reward_index,
        old_reward_duration: wire.old_reward_duration,
        new_reward_duration: wire.new_reward_duration,
    }
}
