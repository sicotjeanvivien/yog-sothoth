use crate::repositories::helper::convert_i64_to_u64;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::NetworkStatus};

/// Row shape for reading `network_status`.
///
/// A thin sqlx-facing struct kept separate from the domain model:
/// it holds the raw `i64` slot and `i32` latency, converted back to
/// `u64` / `u32` in the `TryFrom` impl below with explicit bounds
/// checking.
#[derive(sqlx::FromRow)]
pub(super) struct NetworkStatusRow {
    pub(super) slot: i64,
    pub(super) rpc_latency_ms: i32,
    pub(super) observed_at: DateTime<Utc>,
}

impl TryFrom<NetworkStatusRow> for NetworkStatus {
    type Error = RepositoryError;

    fn try_from(row: NetworkStatusRow) -> Result<Self, Self::Error> {
        Ok(NetworkStatus {
            slot: convert_i64_to_u64(row.slot, "slot")?,
            rpc_latency_ms: u32::try_from(row.rpc_latency_ms).map_err(|_| {
                RepositoryError::Integrity(format!(
                    "invalid rpc_latency_ms: {}",
                    row.rpc_latency_ms
                ))
            })?,
            observed_at: row.observed_at,
        })
    }
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
