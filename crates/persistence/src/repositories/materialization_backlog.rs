//! Postgres implementation of [`MaterializationBacklogRepository`].
//!
//! One call to `yog_cagg_materialization_backlog()` (migration 013), which
//! walks TimescaleDB's catalog: which aggregates exist, and which raw table
//! each summarises, is the database's knowledge, not a list kept here.
//!
//! [`MaterializationBacklogRepository`]: yog_core::domain::MaterializationBacklogRepository

mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::MaterializationBacklogRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{MaterializationBacklog, MaterializationBacklogRepository},
};

/// Postgres-backed materialisation backlog repository.
#[derive(Clone)]
pub struct PgMaterializationBacklogRepository {
    pool: PgPool,
}

impl PgMaterializationBacklogRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MaterializationBacklogRepository for PgMaterializationBacklogRepository {
    async fn backlogs(&self) -> RepositoryResult<Vec<MaterializationBacklog>> {
        // A set-returning function's columns all read as nullable to sqlx:
        // `aggregate` is never NULL (a view name), the two timestamps are
        // NULL by design.
        let rows = sqlx::query_as!(
            MaterializationBacklogRow,
            r#"
            SELECT aggregate AS "aggregate!",
                   watermark AS "watermark?: chrono::DateTime<chrono::Utc>",
                   oldest_pending_at AS "oldest_pending_at?: chrono::DateTime<chrono::Utc>"
              FROM yog_cagg_materialization_backlog()
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows.into_iter().map(MaterializationBacklog::from).collect())
    }
}
