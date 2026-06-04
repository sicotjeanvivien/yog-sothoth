//! Postgres implementation of `EventFreshnessRepository`.
//!
//! A single query: the greatest `timestamp` across `swap_events` and
//! `liquidity_events`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use yog_core::{RepositoryResult, domain::EventFreshnessRepository};

use crate::repository_utils::map_sqlx_error;

/// Postgres-backed event freshness repository.
#[derive(Clone)]
pub struct PgEventFreshnessRepository {
    pool: PgPool,
}

impl PgEventFreshnessRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventFreshnessRepository for PgEventFreshnessRepository {
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        // GREATEST over the two per-table maxima. Each MAX is NULL on
        // an empty table; GREATEST ignores NULLs unless every argument
        // is NULL, in which case the whole expression is NULL — hence
        // the `?` annotation forcing the column to nullable.
        let last_event_at = sqlx::query_scalar!(
            r#"
            SELECT GREATEST(
                (SELECT MAX(timestamp) FROM swap_events),
                (SELECT MAX(timestamp) FROM liquidity_events)
            ) AS "last_event_at?: chrono::DateTime<chrono::Utc>"
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(last_event_at)
    }
}
