//! Postgres implementation of `MaterializationRepository`.
//!
//! One call to `yog_cagg_materialization_progress()` (migration 013), which
//! walks TimescaleDB's catalog: which aggregates exist, and which raw table
//! each summarises, is the database's knowledge, not a list kept here.

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{AggregateMaterialization, MaterializationRepository},
};

/// Postgres-backed materialisation progress repository.
#[derive(Clone)]
pub struct PgMaterializationRepository {
    pool: PgPool,
}

impl PgMaterializationRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MaterializationRepository for PgMaterializationRepository {
    async fn progress(&self) -> RepositoryResult<Vec<AggregateMaterialization>> {
        // A set-returning function's columns all read as nullable to sqlx:
        // `aggregate` is never NULL (a view name), the two timestamps are
        // NULL by design.
        sqlx::query_as!(
            AggregateMaterialization,
            r#"
            SELECT aggregate AS "aggregate!",
                   watermark AS "watermark?: chrono::DateTime<chrono::Utc>",
                   oldest_pending_at AS "oldest_pending_at?: chrono::DateTime<chrono::Utc>"
              FROM yog_cagg_materialization_progress()
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}
