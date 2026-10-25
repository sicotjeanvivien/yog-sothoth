//! Postgres implementation of `AnnouncementLookup`, read-only: an announcement
//! is published by an operator INSERT.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{AnnouncementEntryRow, AnnouncementRow};
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        Announcement, AnnouncementCursor, AnnouncementEntry, AnnouncementLookup, AnnouncementStatus,
    },
};

use crate::repositories::helper::{PageBuilder, QueryMode, map_sqlx_error};

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

    async fn list_entries(
        &self,
        status: Option<AnnouncementStatus>,
        now: DateTime<Utc>,
        cursor: Option<AnnouncementCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<AnnouncementEntry>> {
        // ⚠️ Computed once: the filter and the item read the same status.
        let rows = sqlx::query_as!(
            AnnouncementEntryRow,
            r#"
            SELECT id AS "id!", kind AS "kind!", severity AS "severity!", message AS "message!",
                   link_url AS "link_url?", starts_at AS "starts_at!", ends_at AS "ends_at?",
                   status AS "status!"
              FROM (
                SELECT id, kind, severity, message, link_url, starts_at, ends_at,
                       CASE
                           WHEN starts_at > $1 THEN 'upcoming'
                           WHEN ends_at IS NOT NULL AND ends_at <= $1 THEN 'closed'
                           ELSE 'open'
                       END AS status
                  FROM announcements
              ) AS entries
             WHERE ($2::TEXT IS NULL OR status = $2)
               AND starts_at <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
               AND (starts_at < COALESCE($3, 'infinity'::TIMESTAMPTZ) OR id < $4)
             ORDER BY starts_at DESC, id DESC
             LIMIT $5
            "#,
            now,
            status.map(|s| s.as_str()),
            cursor.as_ref().map(|c| c.starts_at),
            cursor.as_ref().map(|c| c.id),
            limit + 1,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let entries = rows
            .into_iter()
            .map(AnnouncementEntry::try_from)
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page = PageBuilder::new(entries, limit, QueryMode::Forward, cursor.is_some()).finalize(
            |entry| {
                Cursor::Announcement(Box::new(AnnouncementCursor {
                    status,
                    starts_at: entry.announcement.starts_at,
                    id: entry.announcement.id,
                }))
            },
        );
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            prev_cursor: None,
            ..page
        })
    }
}
