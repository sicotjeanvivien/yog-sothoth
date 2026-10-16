//! Postgres implementation of `NetworkStatusRepository`, over the singleton row
//! `id = 1` of `network_status`.
mod rows;

use crate::repositories::helper::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use rows::NetworkStatusRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{NetworkStatus, NetworkStatusLookup, NetworkStatusRepository},
};

#[derive(Clone)]
pub struct PgNetworkStatusRepository {
    pool: PgPool,
}

impl PgNetworkStatusRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NetworkStatusRepository for PgNetworkStatusRepository {
    async fn upsert(&self, status: &NetworkStatus) -> RepositoryResult<()> {
        let slot = convert_u64_to_i64(status.slot, "slot")?;
        let latency = i32::try_from(status.rpc_latency_ms).map_err(|_| {
            RepositoryError::Integrity(format!("invalid rpc_latency_ms: {}", status.rpc_latency_ms))
        })?;

        sqlx::query!(
            r#"
            INSERT INTO network_status (id, slot, rpc_latency_ms, observed_at)
            VALUES (1, $1, $2, $3)
            ON CONFLICT (id) DO UPDATE
            SET slot           = EXCLUDED.slot,
                rpc_latency_ms = EXCLUDED.rpc_latency_ms,
                observed_at    = EXCLUDED.observed_at
            "#,
            slot,
            latency,
            status.observed_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }
}

#[async_trait]
impl NetworkStatusLookup for PgNetworkStatusRepository {
    async fn get(&self) -> RepositoryResult<Option<NetworkStatus>> {
        // The migration seeds the row: None means the seed is missing.
        let row = sqlx::query_as!(
            NetworkStatusRow,
            r#"
            SELECT slot, rpc_latency_ms, observed_at
            FROM network_status
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(NetworkStatus::try_from).transpose()
    }
}
