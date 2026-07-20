//! Postgres implementation of `AnnouncementLookup`.
//!
//! Backed by the `announcements` operator table (migration 026). Read
//! path only — publication is an operator INSERT via psql (the api
//! connects as the read-only `yog_api` role).

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::AnnouncementRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{Announcement, AnnouncementLookup},
};

use crate::repositories::helper::map_sqlx_error;

/// Safety bound on the active-window read. The table is operator-curated
/// (a handful of rows); the limit only guards against a runaway INSERT
/// script, it is not pagination.
const ACTIVE_LIMIT: i64 = 10;

/// Postgres-backed announcement read repository.
#[derive(Clone)]
pub struct PgAnnouncementRepository {
    pool: PgPool,
}

impl PgAnnouncementRepository {
    /// Build the repository over a shared connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnnouncementLookup for PgAnnouncementRepository {
    async fn list_active(&self, now: DateTime<Utc>) -> RepositoryResult<Vec<Announcement>> {
        // Most severe first (the CASE mirrors the enum's escalation
        // order — TEXT would sort alphabetically), then most recent.
        let rows = sqlx::query_as!(
            AnnouncementRow,
            r#"
            SELECT id, kind, severity, message, link_url, starts_at, ends_at
            FROM announcements
            WHERE starts_at <= $1
              AND (ends_at IS NULL OR ends_at > $1)
            ORDER BY
                CASE severity
                    WHEN 'critical' THEN 0
                    WHEN 'warning'  THEN 1
                    ELSE 2
                END,
                starts_at DESC
            LIMIT $2
            "#,
            now,
            ACTIVE_LIMIT,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(Announcement::try_from).collect()
    }
}
