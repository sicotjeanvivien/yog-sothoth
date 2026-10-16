//! Postgres implementation of [`MaterializationBacklogRepository`]: one call to
//! `yog_cagg_materialization_backlog()`, which reads the aggregates from TimescaleDB's catalog.
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

#[derive(Clone)]
pub struct PgMaterializationBacklogRepository {
    pool: PgPool,
}

impl PgMaterializationBacklogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MaterializationBacklogRepository for PgMaterializationBacklogRepository {
    async fn backlogs(&self) -> RepositoryResult<Vec<MaterializationBacklog>> {
        // sqlx reads every column of a set-returning function as nullable: only
        // `aggregate` is never NULL.
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
