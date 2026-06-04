use crate::repository_utils::convert_string_to_pubkey;
use chrono::{DateTime, Utc};
use std::str::FromStr;
use yog_core::{
    RepositoryError,
    domain::{Protocol, WatchedPool},
};

/// Row shape for reading `watched_pools`. Mirrors every column of
/// the table.
#[derive(sqlx::FromRow)]
pub(super) struct WatchedPoolRow {
    pub(super) pool_address: String,
    pub(super) protocol: String,
    pub(super) active: bool,
    pub(super) added_at: DateTime<Utc>,
    pub(super) note: Option<String>,
}

impl TryFrom<WatchedPoolRow> for WatchedPool {
    type Error = RepositoryError;

    fn try_from(row: WatchedPoolRow) -> Result<Self, Self::Error> {
        Ok(WatchedPool {
            pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
            protocol: Protocol::from_str(&row.protocol)
                .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
            active: row.active,
            added_at: row.added_at,
            note: row.note,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
