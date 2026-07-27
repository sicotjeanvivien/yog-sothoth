//! Translation of `cp-amm::EvtFundReward` into its domain event.

use chrono::{DateTime, Utc};
use solana_signature::Signature;

use crate::application::extraction::meteora::damm_v2::events::EvtFundReward;
use crate::domain::MeteoraDammV2FundRewardEvent;

/// Translate an [`EvtFundReward`] into a [`MeteoraDammV2FundRewardEvent`].
///
/// This translation is infallible — every field maps directly. The Q64.64 rate
/// pair is carried through unscaled; interpreting it is the reader's job.
pub(super) fn translate_fund_reward(
    wire: &EvtFundReward,
    signature: Signature,
    timestamp: DateTime<Utc>,
) -> MeteoraDammV2FundRewardEvent {
    MeteoraDammV2FundRewardEvent {
        pool_address: wire.pool,
        signature,
        timestamp,
        funder: wire.funder,
        mint_reward: wire.mint_reward,
        reward_index: wire.reward_index,
        amount: wire.amount,
        transfer_fee_excluded_amount_in: wire.transfer_fee_excluded_amount_in,
        reward_duration_end: wire.reward_duration_end,
        pre_reward_rate: wire.pre_reward_rate,
        post_reward_rate: wire.post_reward_rate,
    }
}
