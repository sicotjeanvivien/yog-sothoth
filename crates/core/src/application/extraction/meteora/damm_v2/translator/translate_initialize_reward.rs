//! Translation of `cp-amm::EvtInitializeReward` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtInitializeReward;
use crate::domain::MeteoraDammV2InitializeRewardEvent;

/// Translate an [`EvtInitializeReward`] into a [`MeteoraDammV2InitializeRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_initialize_reward(
    wire: &EvtInitializeReward,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2InitializeRewardEvent {
    MeteoraDammV2InitializeRewardEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        reward_mint: wire.reward_mint,
        funder: wire.funder,
        creator: wire.creator,
        reward_index: wire.reward_index,
        reward_duration: wire.reward_duration,
    }
}
