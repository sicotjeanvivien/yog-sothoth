use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{LiquidityEvent, Protocol},
};

use crate::repository_utils::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey,
    convert_string_to_signature, parse_string_to_liquidity_event_kind,
};

/// Row shape returned by SELECTs on `liquidity_events`. Mirrors every
/// column of the table; used by `find_by_pool_paginated` in both
/// traversal modes.
#[derive(sqlx::FromRow)]
pub(super) struct LiquidityEventRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) signature: String,
    pub(super) timestamp: DateTime<Utc>,
    pub(super) token_a_mint: String,
    pub(super) token_b_mint: String,
    pub(super) liquidity_event_kind: String,
    pub(super) amount_a: i64,
    pub(super) amount_b: i64,
    pub(super) liquidity_delta: BigDecimal,
    pub(super) reserve_a_after: i64,
    pub(super) reserve_b_after: i64,
    pub(super) position: String,
    pub(super) owner: String,
}

impl TryFrom<LiquidityEventRow> for LiquidityEvent {
    type Error = RepositoryError;

    fn try_from(row: LiquidityEventRow) -> Result<Self, Self::Error> {
        Ok(LiquidityEvent {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            signature: convert_string_to_signature(row.signature, "signature")?,
            timestamp: row.timestamp,
            token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
            token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
            liquidity_event_kind: parse_string_to_liquidity_event_kind(
                row.liquidity_event_kind,
                "liquidity_event_kind",
            )?,
            amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
            amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
            liquidity_delta: convert_bigdecimal_to_u128(row.liquidity_delta, "liquidity_delta")?,
            reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
            reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
            position: convert_string_to_pubkey(row.position, "position")?,
            owner: convert_string_to_pubkey(row.owner, "owner")?,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
