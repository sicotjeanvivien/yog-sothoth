use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{MeteoraDammV2SwapEvent, TradeDirection},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey,
    convert_string_to_signature,
};

/// Row shape returned by SELECTs on `swap_events`. Mirrors every
/// column of the table; used by `find_by_pool_paginated` in both
/// traversal modes.
#[derive(sqlx::FromRow)]
pub(super) struct MeteoraDammV2SwapEventRow {
    pub(super) pool_address: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) trade_direction: String,
    pub(super) amount_a: i64,
    pub(super) amount_b: i64,
    pub(super) reserve_a_after: i64,
    pub(super) reserve_b_after: i64,
    pub(super) next_sqrt_price: BigDecimal,
    pub(super) claiming_fee: i64,
    pub(super) protocol_fee: i64,
    pub(super) compounding_fee: i64,
    pub(super) referral_fee: i64,
    pub(super) fee_token_is_a: bool,
}

impl TryFrom<MeteoraDammV2SwapEventRow> for MeteoraDammV2SwapEvent {
    type Error = RepositoryError;

    fn try_from(row: MeteoraDammV2SwapEventRow) -> Result<Self, Self::Error> {
        Ok(MeteoraDammV2SwapEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            trade_direction: TradeDirection::from_str(&row.trade_direction).map_err(|_| {
                RepositoryError::Integrity(format!(
                    "invalid trade_direction: {}",
                    row.trade_direction
                ))
            })?,
            amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
            amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
            reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
            reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
            next_sqrt_price: convert_bigdecimal_to_u128(row.next_sqrt_price, "next_sqrt_price")?,
            claiming_fee: convert_i64_to_u64(row.claiming_fee, "claiming_fee")?,
            protocol_fee: convert_i64_to_u64(row.protocol_fee, "protocol_fee")?,
            compounding_fee: convert_i64_to_u64(row.compounding_fee, "compounding_fee")?,
            referral_fee: convert_i64_to_u64(row.referral_fee, "referral_fee")?,
            fee_token_is_a: row.fee_token_is_a,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
