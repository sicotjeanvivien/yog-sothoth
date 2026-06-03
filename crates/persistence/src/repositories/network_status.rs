//! Postgres implementation of `NetworkStatusRepository`.
//!
//! Backed by the singleton `network_status` table (see migration
//! 003). Both operations target the single row `id = 1`.
mod rows;

use crate::repository_utils::{convert_u64_to_i64, map_sqlx_error};
use async_trait::async_trait;
use rows::NetworkStatusRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{NetworkStatus, NetworkStatusRepository},
};

/// Postgres-backed network status repository.
#[derive(Clone)]
pub struct PgNetworkStatusRepository {
    pool: PgPool,
}

impl PgNetworkStatusRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NetworkStatusRepository for PgNetworkStatusRepository {
    async fn upsert(&self, status: &NetworkStatus) -> RepositoryResult<()> {
        // Postgres has no unsigned integers; check the casts rather
        // than `as`-ing them silently.
        let slot = convert_u64_to_i64(status.slot, "slot")?;
        let latency = i32::try_from(status.rpc_latency_ms).map_err(|_| {
            RepositoryError::Integrity(format!("invalid rpc_latency_ms: {}", status.rpc_latency_ms))
        })?;

        // Singleton upsert: the row id is always 1. ON CONFLICT keeps
        // it a single row no matter how often the indexer ticks.
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

    async fn get(&self) -> RepositoryResult<Option<NetworkStatus>> {
        // The singleton row is seeded by the migration, so this
        // normally returns Some. None means the seed row is missing.
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
