use crate::repositories::helper::convert_i64_to_u64;
use chrono::{DateTime, Utc};
use yog_core::{RepositoryError, domain::NetworkStatus};

/// Row shape of `network_status`.
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
