use chrono::{DateTime, Datelike, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use yog_core::domain::{
    MeteoraDammV2FundRewardEvent, MeteoraDammV2InitializeRewardEvent, MeteoraDammV2RewardChange,
};

use crate::http::dto::schema::U128String;

/// What a DAMM v2 reward change is, named by `kind`, and what that change says.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum RewardChangeEventMeteoraDammV2Response {
    Initialize(MeteoraDammV2RewardInitializeResponse),
    Fund(MeteoraDammV2RewardFundResponse),
    DurationChange(MeteoraDammV2RewardDurationChangeResponse),
    FunderChange(MeteoraDammV2RewardFunderChangeResponse),
    IneligibleWithdrawal(MeteoraDammV2RewardWithdrawalResponse),
    DeadLiquidityWithdrawal(MeteoraDammV2RewardWithdrawalResponse),
}

/// A reward set up on its pool: its mint, who funds it, who created it, its rank and the length
/// of a funding window, in seconds.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2RewardInitializeResponse {
    #[schema(min_length = 1)]
    reward_mint: String,
    #[schema(min_length = 1)]
    funder: String,
    #[schema(min_length = 1)]
    creator: String,
    reward_index: u8,
    #[schema(value_type = U128String)]
    reward_duration: String,
}

/// A reward funded: what the funder sent and what reached the vault, the end of the funding
/// window, and the emission rate before and after, Q64.64.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2RewardFundResponse {
    #[schema(min_length = 1)]
    funder: String,
    #[schema(min_length = 1)]
    reward_mint: String,
    reward_index: u8,
    #[schema(value_type = U128String)]
    amount: String,
    #[schema(value_type = U128String)]
    transfer_fee_excluded_amount_in: String,
    /// `null` when the chain's seconds fall past the year 9999, which RFC 3339 cannot write.
    reward_duration_end: Option<DateTime<Utc>>,
    #[schema(value_type = U128String)]
    pre_reward_rate: String,
    #[schema(value_type = U128String)]
    post_reward_rate: String,
}

/// A reward's funding window changed, in seconds.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2RewardDurationChangeResponse {
    reward_index: u8,
    #[schema(value_type = U128String)]
    old_reward_duration: String,
    #[schema(value_type = U128String)]
    new_reward_duration: String,
}

/// A reward's funder changed.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2RewardFunderChangeResponse {
    reward_index: u8,
    #[schema(min_length = 1)]
    old_funder: String,
    #[schema(min_length = 1)]
    new_funder: String,
}

/// A reward amount withdrawn from the pool: the mint and what left.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct MeteoraDammV2RewardWithdrawalResponse {
    #[schema(min_length = 1)]
    reward_mint: String,
    #[schema(value_type = U128String)]
    amount: String,
}

impl From<MeteoraDammV2RewardChange> for RewardChangeEventMeteoraDammV2Response {
    fn from(change: MeteoraDammV2RewardChange) -> Self {
        match change {
            MeteoraDammV2RewardChange::Initialize(e) => Self::Initialize(e.into()),
            MeteoraDammV2RewardChange::Fund(e) => Self::Fund(e.into()),
            MeteoraDammV2RewardChange::DurationChange(e) => {
                Self::DurationChange(MeteoraDammV2RewardDurationChangeResponse {
                    reward_index: e.reward_index,
                    old_reward_duration: e.old_reward_duration.to_string(),
                    new_reward_duration: e.new_reward_duration.to_string(),
                })
            }
            MeteoraDammV2RewardChange::FunderChange(e) => {
                Self::FunderChange(MeteoraDammV2RewardFunderChangeResponse {
                    reward_index: e.reward_index,
                    old_funder: e.old_funder.to_string(),
                    new_funder: e.new_funder.to_string(),
                })
            }
            MeteoraDammV2RewardChange::IneligibleWithdrawal(e) => {
                Self::IneligibleWithdrawal(MeteoraDammV2RewardWithdrawalResponse {
                    reward_mint: e.reward_mint.to_string(),
                    amount: e.amount.to_string(),
                })
            }
            MeteoraDammV2RewardChange::DeadLiquidityWithdrawal(e) => {
                Self::DeadLiquidityWithdrawal(MeteoraDammV2RewardWithdrawalResponse {
                    reward_mint: e.reward_mint.to_string(),
                    amount: e.amount.to_string(),
                })
            }
        }
    }
}

impl From<MeteoraDammV2InitializeRewardEvent> for MeteoraDammV2RewardInitializeResponse {
    fn from(e: MeteoraDammV2InitializeRewardEvent) -> Self {
        Self {
            reward_mint: e.reward_mint.to_string(),
            funder: e.funder.to_string(),
            creator: e.creator.to_string(),
            reward_index: e.reward_index,
            reward_duration: e.reward_duration.to_string(),
        }
    }
}

impl From<MeteoraDammV2FundRewardEvent> for MeteoraDammV2RewardFundResponse {
    fn from(e: MeteoraDammV2FundRewardEvent) -> Self {
        Self {
            funder: e.funder.to_string(),
            reward_mint: e.mint_reward.to_string(),
            reward_index: e.reward_index,
            amount: e.amount.to_string(),
            transfer_fee_excluded_amount_in: e.transfer_fee_excluded_amount_in.to_string(),
            reward_duration_end: i64::try_from(e.reward_duration_end)
                .ok()
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
                .filter(|end| end.year() <= 9999),
            pre_reward_rate: e.pre_reward_rate.to_string(),
            post_reward_rate: e.post_reward_rate.to_string(),
        }
    }
}
