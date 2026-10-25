use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{
        MeteoraDammV2FundRewardEvent, MeteoraDammV2InitializeRewardEvent,
        MeteoraDammV2UpdateRewardDurationEvent, MeteoraDammV2UpdateRewardFunderEvent,
        MeteoraDammV2WithdrawDeadLiquidityRewardEvent, MeteoraDammV2WithdrawIneligibleRewardEvent,
    },
};

use crate::repositories::helper::{
    convert_bigdecimal_to_u128, convert_i16_to_u8, convert_i32_to_u16, convert_i64_to_u32,
    convert_i64_to_u64, convert_optional, convert_string_to_pubkey, convert_string_to_signature,
};

/// A row of `meteora_damm_v2_initialize_reward_events`, as the list reads it.
pub(super) struct MeteoraDammV2InitializeRewardEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) reward_mint: String,
    pub(super) funder: String,
    pub(super) creator: String,
    pub(super) reward_index: i16,
    pub(super) reward_duration: i64,
}

impl TryFrom<MeteoraDammV2InitializeRewardEventRow> for MeteoraDammV2InitializeRewardEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2InitializeRewardEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2InitializeRewardEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            reward_mint: convert_string_to_pubkey(row.reward_mint, "reward_mint")?,
            funder: convert_string_to_pubkey(row.funder, "funder")?,
            creator: convert_string_to_pubkey(row.creator, "creator")?,
            reward_index: convert_i16_to_u8(row.reward_index, "reward_index")?,
            reward_duration: convert_i64_to_u64(row.reward_duration, "reward_duration")?,
        })
    }
}

/// A row of `meteora_damm_v2_fund_reward_events`, as the list reads it.
pub(super) struct MeteoraDammV2FundRewardEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) funder: String,
    pub(super) mint_reward: String,
    pub(super) reward_index: i16,
    pub(super) amount: i64,
    pub(super) transfer_fee_excluded_amount_in: i64,
    pub(super) reward_duration_end: i64,
    pub(super) pre_reward_rate: BigDecimal,
    pub(super) post_reward_rate: BigDecimal,
}

impl TryFrom<MeteoraDammV2FundRewardEventRow> for MeteoraDammV2FundRewardEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2FundRewardEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2FundRewardEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            funder: convert_string_to_pubkey(row.funder, "funder")?,
            mint_reward: convert_string_to_pubkey(row.mint_reward, "mint_reward")?,
            reward_index: convert_i16_to_u8(row.reward_index, "reward_index")?,
            amount: convert_i64_to_u64(row.amount, "amount")?,
            transfer_fee_excluded_amount_in: convert_i64_to_u64(
                row.transfer_fee_excluded_amount_in,
                "transfer_fee_excluded_amount_in",
            )?,
            reward_duration_end: convert_i64_to_u64(
                row.reward_duration_end,
                "reward_duration_end",
            )?,
            pre_reward_rate: convert_bigdecimal_to_u128(row.pre_reward_rate, "pre_reward_rate")?,
            post_reward_rate: convert_bigdecimal_to_u128(row.post_reward_rate, "post_reward_rate")?,
        })
    }
}

/// A row of `meteora_damm_v2_update_reward_duration_events`, as the list reads it.
pub(super) struct MeteoraDammV2UpdateRewardDurationEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) reward_index: i16,
    pub(super) old_reward_duration: i64,
    pub(super) new_reward_duration: i64,
}

impl TryFrom<MeteoraDammV2UpdateRewardDurationEventRow> for MeteoraDammV2UpdateRewardDurationEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2UpdateRewardDurationEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2UpdateRewardDurationEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            reward_index: convert_i16_to_u8(row.reward_index, "reward_index")?,
            old_reward_duration: convert_i64_to_u64(
                row.old_reward_duration,
                "old_reward_duration",
            )?,
            new_reward_duration: convert_i64_to_u64(
                row.new_reward_duration,
                "new_reward_duration",
            )?,
        })
    }
}

/// A row of `meteora_damm_v2_update_reward_funder_events`, as the list reads it.
pub(super) struct MeteoraDammV2UpdateRewardFunderEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) reward_index: i16,
    pub(super) old_funder: String,
    pub(super) new_funder: String,
}

impl TryFrom<MeteoraDammV2UpdateRewardFunderEventRow> for MeteoraDammV2UpdateRewardFunderEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2UpdateRewardFunderEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2UpdateRewardFunderEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            reward_index: convert_i16_to_u8(row.reward_index, "reward_index")?,
            old_funder: convert_string_to_pubkey(row.old_funder, "old_funder")?,
            new_funder: convert_string_to_pubkey(row.new_funder, "new_funder")?,
        })
    }
}

/// A row of `meteora_damm_v2_withdraw_ineligible_reward_events`, as the list reads it.
pub(super) struct MeteoraDammV2WithdrawIneligibleRewardEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) reward_mint: String,
    pub(super) amount: i64,
}

impl TryFrom<MeteoraDammV2WithdrawIneligibleRewardEventRow>
    for MeteoraDammV2WithdrawIneligibleRewardEvent
{
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2WithdrawIneligibleRewardEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2WithdrawIneligibleRewardEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            reward_mint: convert_string_to_pubkey(row.reward_mint, "reward_mint")?,
            amount: convert_i64_to_u64(row.amount, "amount")?,
        })
    }
}

/// A row of `meteora_damm_v2_withdraw_dead_liquidity_reward_events`, as the list reads it.
pub(super) struct MeteoraDammV2WithdrawDeadLiquidityRewardEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) reward_mint: String,
    pub(super) amount: i64,
}

impl TryFrom<MeteoraDammV2WithdrawDeadLiquidityRewardEventRow>
    for MeteoraDammV2WithdrawDeadLiquidityRewardEvent
{
    type Error = RepositoryError;

    fn try_from(
        row: MeteoraDammV2WithdrawDeadLiquidityRewardEventRow,
    ) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2WithdrawDeadLiquidityRewardEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            slot: convert_i64_to_u64(row.slot, "slot")?,
            transaction_index: convert_optional(
                row.transaction_index,
                "transaction_index",
                convert_i64_to_u32,
            )?,
            event_index: convert_i32_to_u16(row.event_index, "event_index")?,
            reward_mint: convert_string_to_pubkey(row.reward_mint, "reward_mint")?,
            amount: convert_i64_to_u64(row.amount, "amount")?,
        })
    }
}
