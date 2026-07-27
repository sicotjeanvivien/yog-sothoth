//! Translation of `cp-amm::EvtUpdateRewardFunder` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtUpdateRewardFunder;
use crate::domain::MeteoraDammV2UpdateRewardFunderEvent;

/// Translate an [`EvtUpdateRewardFunder`] into a
/// [`MeteoraDammV2UpdateRewardFunderEvent`]. Infallible.
pub(super) fn translate_update_reward_funder(
    wire: &EvtUpdateRewardFunder,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2UpdateRewardFunderEvent {
    MeteoraDammV2UpdateRewardFunderEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        reward_index: wire.reward_index,
        old_funder: wire.old_funder,
        new_funder: wire.new_funder,
    }
}
