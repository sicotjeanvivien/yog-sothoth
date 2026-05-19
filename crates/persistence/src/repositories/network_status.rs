//! Postgres implementation of `NetworkStatusRepository`.
//!
//! Backed by the singleton `network_status` table (see migration
//! 003). Both operations target the single row `id = 1`.
//!
//! NOTE: adjust the error type / pool field to match the conventions
//! of the other repositories in this crate (e.g. `pool_current_state.rs`)
//! if they differ — this follows the expected shared pattern: a
//! `PgPool` injected at construction and a crate-wide `RepositoryError`.

use async_trait::async_trait;
use sqlx::PgPool;

use yog_core::{
    RepositoryResult,
    domain::{NetworkStatus, NetworkStatusRepository},
};

use crate::repository_utils::map_sqlx_error;

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
        // Postgres has no unsigned integers: slot is u64 on-chain,
        // stored as BIGINT. The value stays well within i64 range
        // (current slots are ~10 digits), so the cast is safe.
        let slot = status.slot as i64;
        let latency = status.rpc_latency_ms as i32;

        // Singleton upsert: the row id is always 1. ON CONFLICT keeps
        // it a single row no matter how often the indexer ticks.
        sqlx::query(
            r#"
            INSERT INTO network_status (id, slot, rpc_latency_ms, observed_at)
            VALUES (1, $1, $2, $3)
            ON CONFLICT (id) DO UPDATE
            SET slot           = EXCLUDED.slot,
                rpc_latency_ms = EXCLUDED.rpc_latency_ms,
                observed_at    = EXCLUDED.observed_at
            "#,
        )
        .bind(slot)
        .bind(latency)
        .bind(status.observed_at)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn get(&self) -> RepositoryResult<Option<NetworkStatus>> {
        // The singleton row is seeded by the migration, so this
        // normally returns Some. None means the seed row is missing.
        let row = sqlx::query_as::<_, NetworkStatusRow>(
            r#"
            SELECT slot, rpc_latency_ms, observed_at
            FROM network_status
            WHERE id = 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(Into::into))
    }
}

/// Row shape for reading `network_status`.
///
/// A thin sqlx-facing struct kept separate from the domain model:
/// it holds the raw `i64` slot, converted back to `u64` in the
/// `From` impl below.
#[derive(sqlx::FromRow)]
struct NetworkStatusRow {
    slot: i64,
    rpc_latency_ms: i32,
    observed_at: chrono::DateTime<chrono::Utc>,
}

impl From<NetworkStatusRow> for NetworkStatus {
    fn from(row: NetworkStatusRow) -> Self {
        NetworkStatus {
            // Reverse of the persist-side cast. Slot values are
            // always non-negative in practice.
            slot: row.slot as u64,
            rpc_latency_ms: row.rpc_latency_ms as u32,
            observed_at: row.observed_at,
        }
    }
}
