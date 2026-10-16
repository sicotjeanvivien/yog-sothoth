//! Postgres implementation of `AnnouncementLookup`, read-only: an announcement
//! is published by an operator INSERT.

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

/// Bound against a runaway INSERT script, not pagination.
const ACTIVE_LIMIT: i64 = 10;

#[derive(Clone)]
pub struct PgAnnouncementRepository {
    pool: PgPool,
}

impl PgAnnouncementRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AnnouncementLookup for PgAnnouncementRepository {
    async fn list_active(&self, now: DateTime<Utc>) -> RepositoryResult<Vec<Announcement>> {
        // Most severe first — a CASE, since TEXT sorts alphabetically — then most recent.
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
