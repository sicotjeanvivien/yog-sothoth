//! Translation of `cp-amm::EvtWithdrawIneligibleReward` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtWithdrawIneligibleReward;
use crate::domain::MeteoraDammV2WithdrawIneligibleRewardEvent;

/// Translate an [`EvtWithdrawIneligibleReward`] into a
/// [`MeteoraDammV2WithdrawIneligibleRewardEvent`].
///
/// This translation is infallible — every field maps directly.
pub(super) fn translate_withdraw_ineligible_reward(
    wire: &EvtWithdrawIneligibleReward,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2WithdrawIneligibleRewardEvent {
    MeteoraDammV2WithdrawIneligibleRewardEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        reward_mint: wire.reward_mint,
        amount: wire.amount,
    }
}
