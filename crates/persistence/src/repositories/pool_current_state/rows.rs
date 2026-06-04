use crate::repositories::helper::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey,
    convert_string_to_signature,
};
use chrono::{DateTime, Utc};
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{LastEventKind, PoolCurrentState, Protocol},
};

/// Raw row mirror — mirrors the SELECT column order below.
#[derive(sqlx::FromRow)]
pub(super) struct PoolCurrentStateRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) last_event_at: DateTime<Utc>,
    pub(super) last_event_kind: String,
    pub(super) last_signature: String,
    pub(super) reserve_a: i64,
    pub(super) reserve_b: i64,
    pub(super) last_sqrt_price: Option<BigDecimal>,
    pub(super) last_swap_at: Option<DateTime<Utc>>,
    pub(super) liquidity: Option<BigDecimal>,
    pub(super) last_liquidity_at: Option<DateTime<Utc>>,
    pub(super) updated_at: DateTime<Utc>,
}

impl TryFrom<PoolCurrentStateRow> for PoolCurrentState {
    type Error = RepositoryError;

    fn try_from(row: PoolCurrentStateRow) -> Result<Self, Self::Error> {
        let last_event_kind = LastEventKind::from_wire(&row.last_event_kind).ok_or_else(|| {
            RepositoryError::Integrity(format!(
                "invalid last_event_kind in pool_current_state: {}",
                row.last_event_kind
            ))
        })?;

        Ok(PoolCurrentState {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            last_event_at: row.last_event_at,
            last_event_kind,
            last_signature: convert_string_to_signature(row.last_signature, "last_signature")?,
            reserve_a: convert_i64_to_u64(row.reserve_a, "reserve_a")?,
            reserve_b: convert_i64_to_u64(row.reserve_b, "reserve_b")?,
            last_sqrt_price: row
                .last_sqrt_price
                .map(|v| convert_bigdecimal_to_u128(v, "last_sqrt_price"))
                .transpose()?,
            last_swap_at: row.last_swap_at,
            liquidity: row
                .liquidity
                .map(|v| convert_bigdecimal_to_u128(v, "liquidity"))
                .transpose()?,
            last_liquidity_at: row.last_liquidity_at,
            updated_at: row.updated_at,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
