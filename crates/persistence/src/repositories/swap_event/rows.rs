use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{Protocol, SwapEvent, TradeDirection},
};

use crate::repositories::helper::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey,
    convert_string_to_signature,
};

/// Row shape returned by SELECTs on `swap_events`. Mirrors every
/// column of the table; used by `find_by_pool_paginated` in both
/// traversal modes.
#[derive(sqlx::FromRow)]
pub(super) struct SwapEventRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) token_a_mint: String,
    pub(super) token_b_mint: String,
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

impl TryFrom<SwapEventRow> for SwapEvent {
    type Error = RepositoryError;

    fn try_from(row: SwapEventRow) -> Result<Self, Self::Error> {
        Ok(SwapEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
            token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
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
