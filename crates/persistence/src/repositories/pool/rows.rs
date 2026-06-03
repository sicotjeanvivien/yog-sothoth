//! Row structs for `pools` reads + their `TryFrom` to domain.
//!
//! `PoolRow` mirrors the columns of `pools` as raw SQL types
//! (`String` for Pubkey/enum, `DateTime<Utc>` for timestamps). The
//! conversion to the domain `Pool` (which uses `Pubkey` and
//! `Protocol`) lives here as the canonical parser. Repositories own
//! the orchestration; this module owns the raw shape and its safe
//! conversion. Parse failures bubble up as `RepositoryError::Integrity`.

use chrono::{DateTime, Utc};
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{Pool, Protocol},
};

use crate::repository_utils::convert_string_to_pubkey;

/// Row shape returned by SELECTs on `pools`. Mirrors every column of
/// the table. Used by `find_by_address` and `find_paginated`.
#[derive(sqlx::FromRow)]
pub(super) struct PoolRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) token_a_mint: String,
    pub(super) token_b_mint: String,
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
            token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
            token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
            first_seen_at: row.first_seen_at,
            last_seen_at: row.last_seen_at,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
