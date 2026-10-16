//! Postgres implementation of `EventFreshnessRepository`: the greatest `timestamp`
//! across `swap_events` and `liquidity_events`.

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use yog_core::{RepositoryResult, domain::EventFreshnessRepository};

#[derive(Clone)]
pub struct PgEventFreshnessRepository {
    pool: PgPool,
}

impl PgEventFreshnessRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventFreshnessRepository for PgEventFreshnessRepository {
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        // NULL when both tables are empty — hence the `?`.
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
