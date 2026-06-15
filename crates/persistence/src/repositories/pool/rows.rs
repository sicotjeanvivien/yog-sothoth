use crate::repositories::helper::convert_string_to_pubkey;
use chrono::{DateTime, Utc};
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
            token_a_mint: row
                .token_a_mint
                .map(|m| convert_string_to_pubkey(m, "token_a_mint"))
                .transpose()?,
            token_b_mint: row
                .token_b_mint
                .map(|m| convert_string_to_pubkey(m, "token_b_mint"))
                .transpose()?,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
