use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use yog_core::{
    RepositoryError,
    domain::{
        MeteoraDammV2ClosePositionEvent, MeteoraDammV2CreatePositionEvent,
        MeteoraDammV2LockPositionEvent, MeteoraDammV2PermanentLockPositionEvent,
        MeteoraDammV2SplitAmounts, MeteoraDammV2SplitNumerators, MeteoraDammV2SplitPositionEvent,
        MeteoraDammV2SplitPositionState,
    },
};

use crate::repositories::helper::{
    convert_bigdecimal_to_u128, convert_i32_to_u16, convert_i64_to_u32, convert_i64_to_u64,
    convert_optional, convert_string_to_pubkey, convert_string_to_signature,
};

/// The key of a change in the merged list, and the table and row it is read from.
pub(super) struct PositionChangeKey {
    pub(super) kind: String,
    pub(super) id: i64,
    pub(super) timestamp: DateTime<Utc>,
}

/// A row of `meteora_damm_v2_create_position_events`, as the list reads it.
pub(super) struct MeteoraDammV2CreatePositionEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) owner: String,
    pub(super) position: String,
    pub(super) position_nft_mint: String,
}

impl TryFrom<MeteoraDammV2CreatePositionEventRow> for MeteoraDammV2CreatePositionEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2CreatePositionEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2CreatePositionEvent {
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
            owner: convert_string_to_pubkey(row.owner, "owner")?,
            position: convert_string_to_pubkey(row.position, "position")?,
            position_nft_mint: convert_string_to_pubkey(
                row.position_nft_mint,
                "position_nft_mint",
            )?,
        })
    }
}

/// A row of `meteora_damm_v2_close_position_events`, as the list reads it.
pub(super) struct MeteoraDammV2ClosePositionEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) owner: String,
    pub(super) position: String,
    pub(super) position_nft_mint: String,
}

impl TryFrom<MeteoraDammV2ClosePositionEventRow> for MeteoraDammV2ClosePositionEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2ClosePositionEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2ClosePositionEvent {
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
            owner: convert_string_to_pubkey(row.owner, "owner")?,
            position: convert_string_to_pubkey(row.position, "position")?,
            position_nft_mint: convert_string_to_pubkey(
                row.position_nft_mint,
                "position_nft_mint",
            )?,
        })
    }
}

/// A row of `meteora_damm_v2_lock_position_events`, as the list reads it.
pub(super) struct MeteoraDammV2LockPositionEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) position: String,
    pub(super) owner: String,
    pub(super) vesting: String,
    pub(super) cliff_point: i64,
    pub(super) period_frequency: i64,
    pub(super) cliff_unlock_liquidity: BigDecimal,
    pub(super) liquidity_per_period: BigDecimal,
    pub(super) number_of_period: i32,
}

impl TryFrom<MeteoraDammV2LockPositionEventRow> for MeteoraDammV2LockPositionEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2LockPositionEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2LockPositionEvent {
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
            position: convert_string_to_pubkey(row.position, "position")?,
            owner: convert_string_to_pubkey(row.owner, "owner")?,
            vesting: convert_string_to_pubkey(row.vesting, "vesting")?,
            cliff_point: convert_i64_to_u64(row.cliff_point, "cliff_point")?,
            period_frequency: convert_i64_to_u64(row.period_frequency, "period_frequency")?,
            cliff_unlock_liquidity: convert_bigdecimal_to_u128(
                row.cliff_unlock_liquidity,
                "cliff_unlock_liquidity",
            )?,
            liquidity_per_period: convert_bigdecimal_to_u128(
                row.liquidity_per_period,
                "liquidity_per_period",
            )?,
            number_of_period: convert_i32_to_u16(row.number_of_period, "number_of_period")?,
        })
    }
}

/// A row of `meteora_damm_v2_permanent_lock_position_events`, as the list reads it.
pub(super) struct MeteoraDammV2PermanentLockPositionEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) position: String,
    pub(super) lock_liquidity_amount: BigDecimal,
    pub(super) total_permanent_locked_liquidity: BigDecimal,
}

impl TryFrom<MeteoraDammV2PermanentLockPositionEventRow>
    for MeteoraDammV2PermanentLockPositionEvent
{
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2PermanentLockPositionEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2PermanentLockPositionEvent {
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
            position: convert_string_to_pubkey(row.position, "position")?,
            lock_liquidity_amount: convert_bigdecimal_to_u128(
                row.lock_liquidity_amount,
                "lock_liquidity_amount",
            )?,
            total_permanent_locked_liquidity: convert_bigdecimal_to_u128(
                row.total_permanent_locked_liquidity,
                "total_permanent_locked_liquidity",
            )?,
        })
    }
}

/// A row of `meteora_damm_v2_split_position_events`, as the list reads it.
pub(super) struct MeteoraDammV2SplitPositionEventRow {
    pub(super) id: i64,
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) slot: i64,
    pub(super) transaction_index: Option<i64>,
    pub(super) event_index: i32,
    pub(super) first_owner: String,
    pub(super) second_owner: String,
    pub(super) first_position: String,
    pub(super) second_position: String,
    pub(super) current_sqrt_price: BigDecimal,
    pub(super) split_permanent_locked_liquidity: BigDecimal,
    pub(super) split_unlocked_liquidity: BigDecimal,
    pub(super) split_vested_liquidity: BigDecimal,
    pub(super) split_fee_a: i64,
    pub(super) split_fee_b: i64,
    pub(super) split_reward_0: i64,
    pub(super) split_reward_1: i64,
    pub(super) first_unlocked_liquidity: BigDecimal,
    pub(super) first_permanent_locked_liquidity: BigDecimal,
    pub(super) first_vested_liquidity: BigDecimal,
    pub(super) first_fee_a: i64,
    pub(super) first_fee_b: i64,
    pub(super) first_reward_0: i64,
    pub(super) first_reward_1: i64,
    pub(super) second_unlocked_liquidity: BigDecimal,
    pub(super) second_permanent_locked_liquidity: BigDecimal,
    pub(super) second_vested_liquidity: BigDecimal,
    pub(super) second_fee_a: i64,
    pub(super) second_fee_b: i64,
    pub(super) second_reward_0: i64,
    pub(super) second_reward_1: i64,
    pub(super) num_unlocked_liquidity: i64,
    pub(super) num_permanent_locked_liquidity: i64,
    pub(super) num_fee_a: i64,
    pub(super) num_fee_b: i64,
    pub(super) num_reward_0: i64,
    pub(super) num_reward_1: i64,
    pub(super) num_inner_vesting_liquidity: i64,
}

impl TryFrom<MeteoraDammV2SplitPositionEventRow> for MeteoraDammV2SplitPositionEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2SplitPositionEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2SplitPositionEvent {
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
            first_owner: convert_string_to_pubkey(row.first_owner, "first_owner")?,
            second_owner: convert_string_to_pubkey(row.second_owner, "second_owner")?,
            first_position: convert_string_to_pubkey(row.first_position, "first_position")?,
            second_position: convert_string_to_pubkey(row.second_position, "second_position")?,
            current_sqrt_price: convert_bigdecimal_to_u128(
                row.current_sqrt_price,
                "current_sqrt_price",
            )?,
            amounts: MeteoraDammV2SplitAmounts {
                permanent_locked_liquidity: convert_bigdecimal_to_u128(
                    row.split_permanent_locked_liquidity,
                    "split_permanent_locked_liquidity",
                )?,
                unlocked_liquidity: convert_bigdecimal_to_u128(
                    row.split_unlocked_liquidity,
                    "split_unlocked_liquidity",
                )?,
                vested_liquidity: convert_bigdecimal_to_u128(
                    row.split_vested_liquidity,
                    "split_vested_liquidity",
                )?,
                fee_a: convert_i64_to_u64(row.split_fee_a, "split_fee_a")?,
                fee_b: convert_i64_to_u64(row.split_fee_b, "split_fee_b")?,
                reward_0: convert_i64_to_u64(row.split_reward_0, "split_reward_0")?,
                reward_1: convert_i64_to_u64(row.split_reward_1, "split_reward_1")?,
            },
            first_position_after: MeteoraDammV2SplitPositionState {
                permanent_locked_liquidity: convert_bigdecimal_to_u128(
                    row.first_permanent_locked_liquidity,
                    "first_permanent_locked_liquidity",
                )?,
                unlocked_liquidity: convert_bigdecimal_to_u128(
                    row.first_unlocked_liquidity,
                    "first_unlocked_liquidity",
                )?,
                vested_liquidity: convert_bigdecimal_to_u128(
                    row.first_vested_liquidity,
                    "first_vested_liquidity",
                )?,
                fee_a: convert_i64_to_u64(row.first_fee_a, "first_fee_a")?,
                fee_b: convert_i64_to_u64(row.first_fee_b, "first_fee_b")?,
                reward_0: convert_i64_to_u64(row.first_reward_0, "first_reward_0")?,
                reward_1: convert_i64_to_u64(row.first_reward_1, "first_reward_1")?,
            },
            second_position_after: MeteoraDammV2SplitPositionState {
                permanent_locked_liquidity: convert_bigdecimal_to_u128(
                    row.second_permanent_locked_liquidity,
                    "second_permanent_locked_liquidity",
                )?,
                unlocked_liquidity: convert_bigdecimal_to_u128(
                    row.second_unlocked_liquidity,
                    "second_unlocked_liquidity",
                )?,
                vested_liquidity: convert_bigdecimal_to_u128(
                    row.second_vested_liquidity,
                    "second_vested_liquidity",
                )?,
                fee_a: convert_i64_to_u64(row.second_fee_a, "second_fee_a")?,
                fee_b: convert_i64_to_u64(row.second_fee_b, "second_fee_b")?,
                reward_0: convert_i64_to_u64(row.second_reward_0, "second_reward_0")?,
                reward_1: convert_i64_to_u64(row.second_reward_1, "second_reward_1")?,
            },
            numerators: MeteoraDammV2SplitNumerators {
                unlocked_liquidity: convert_i64_to_u32(
                    row.num_unlocked_liquidity,
                    "num_unlocked_liquidity",
                )?,
                permanent_locked_liquidity: convert_i64_to_u32(
                    row.num_permanent_locked_liquidity,
                    "num_permanent_locked_liquidity",
                )?,
                fee_a: convert_i64_to_u32(row.num_fee_a, "num_fee_a")?,
                fee_b: convert_i64_to_u32(row.num_fee_b, "num_fee_b")?,
                reward_0: convert_i64_to_u32(row.num_reward_0, "num_reward_0")?,
                reward_1: convert_i64_to_u32(row.num_reward_1, "num_reward_1")?,
                inner_vesting_liquidity: convert_i64_to_u32(
                    row.num_inner_vesting_liquidity,
                    "num_inner_vesting_liquidity",
                )?,
            },
        })
    }
}
