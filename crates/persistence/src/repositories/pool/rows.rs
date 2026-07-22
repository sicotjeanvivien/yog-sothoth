use crate::repositories::helper::convert_string_to_pubkey;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{Pool, Protocol},
};

/// Row shape returned by SELECTs on `pools`. Mirrors every column of
/// the table. Used by `find_by_address` and `find_paginated`.
#[derive(sqlx::FromRow)]
pub(super) struct PoolRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) token_a_mint: Option<String>,
    pub(super) token_b_mint: Option<String>,
    pub(super) fee_bps: Option<Decimal>,
    pub(super) protocol_fee_percent: Option<i16>,
    pub(super) partner_fee_percent: Option<i16>,
    pub(super) referral_fee_percent: Option<i16>,
    pub(super) base_fee_kind: Option<String>,
    pub(super) has_dynamic_fee: Option<bool>,
    pub(super) first_seen_at: DateTime<Utc>,
    pub(super) last_seen_at: DateTime<Utc>,
}

/// Convert a SMALLINT fee-split percent back to the domain `u8`. The column
/// only ever holds values written from a `u8` (0..=100), but guard the range
/// rather than silently truncate a corrupt row — surfaces as `Integrity`.
fn percent_to_u8(value: Option<i16>, field: &str) -> Result<Option<u8>, RepositoryError> {
    value
        .map(|v| {
            u8::try_from(v)
                .map_err(|_| RepositoryError::Integrity(format!("{field} out of u8 range: {v}")))
        })
        .transpose()
}

impl TryFrom<PoolRow> for Pool {
    type Error = RepositoryError;

    fn try_from(row: PoolRow) -> Result<Self, Self::Error> {
        Ok(Pool {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            token_a_mint: row
                .token_a_mint
                .map(|m| convert_string_to_pubkey(m, "token_a_mint"))
                .transpose()?,
            token_b_mint: row
                .token_b_mint
                .map(|m| convert_string_to_pubkey(m, "token_b_mint"))
                .transpose()?,
            fee_bps: row.fee_bps,
            protocol_fee_percent: percent_to_u8(row.protocol_fee_percent, "protocol_fee_percent")?,
            partner_fee_percent: percent_to_u8(row.partner_fee_percent, "partner_fee_percent")?,
            referral_fee_percent: percent_to_u8(row.referral_fee_percent, "referral_fee_percent")?,
            base_fee_kind: row.base_fee_kind,
            has_dynamic_fee: row.has_dynamic_fee,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
