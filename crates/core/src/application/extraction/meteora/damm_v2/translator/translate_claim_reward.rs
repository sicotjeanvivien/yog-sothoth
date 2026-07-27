//! Translation of `cp-amm::EvtClaimReward` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtClaimReward;
use crate::domain::MeteoraDammV2ClaimRewardEvent;

/// Translate an [`EvtClaimReward`] into a [`MeteoraDammV2ClaimRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_claim_reward(
    wire: &EvtClaimReward,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2ClaimRewardEvent {
    MeteoraDammV2ClaimRewardEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        position: wire.position,
        owner: wire.owner,
        mint_reward: wire.mint_reward,
        reward_index: wire.reward_index,
        total_reward: wire.total_reward,
    }
}
