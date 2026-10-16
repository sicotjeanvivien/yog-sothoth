use crate::repositories::helper::{convert_optional, convert_string_to_pubkey};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{Pool, Protocol},
};

/// Row shape of `pools`. Per-protocol properties are read from their satellites,
/// such as [`PgMeteoraDammV2PoolPropertiesRepository`](crate::PgMeteoraDammV2PoolPropertiesRepository).
#[derive(sqlx::FromRow)]
pub(super) struct PoolRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) token_a_mint: Option<String>,
    pub(super) token_b_mint: Option<String>,
    pub(super) fee_bps: Option<Decimal>,
    pub(super) first_seen_at: DateTime<Utc>,
    pub(super) last_seen_at: DateTime<Utc>,
}

impl TryFrom<PoolRow> for Pool {
    type Error = RepositoryError;

    fn try_from(row: PoolRow) -> Result<Self, Self::Error> {
        Ok(Pool {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            token_a_mint: convert_optional(
                row.token_a_mint,
                "token_a_mint",
                convert_string_to_pubkey,
            )?,
            token_b_mint: convert_optional(
                row.token_b_mint,
                "token_b_mint",
                convert_string_to_pubkey,
            )?,
            fee_bps: row.fee_bps,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
