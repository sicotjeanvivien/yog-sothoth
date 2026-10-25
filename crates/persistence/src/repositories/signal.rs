//! Postgres implementation of [`SignalRepository`] (append-only INSERT and the
//! dedup read) and [`SignalFeed`] (keyset pagination, one static query per mode).
//!
//! [`SignalRepository`]: yog_core::domain::SignalRepository
//! [`SignalFeed`]: yog_core::domain::SignalFeed

mod rows;

use std::collections::HashMap;
use std::str::FromStr;

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_string_to_pubkey, map_sqlx_error, resolve_query_mode,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::SignalRow;
use solana_pubkey::Pubkey;
use sqlx::{PgPool, QueryBuilder};
use yog_core::{
    Cursor, Page, PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{
        Severity, Signal, SignalCursor, SignalFeed, SignalFilter, SignalRecord, SignalRepository,
    },
};

/// Page size cap, whatever the caller's `limit`.
const MAX_PAGE_SIZE: i64 = 200;

#[derive(Clone)]
pub struct PgSignalRepository {
    pool: PgPool,
}

impl PgSignalRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SignalRepository for PgSignalRepository {
    async fn insert_batch(&self, signals: &[Signal]) -> RepositoryResult<()> {
        if signals.is_empty() {
            return Ok(());
        }

        // QueryBuilder: the `query!` macros cannot build a runtime-sized VALUES list.
        let mut builder = QueryBuilder::new(
            "INSERT INTO signals \
             (detector, protocol, pool_address, severity, value, threshold, message, triggered_at) ",
        );

        builder.push_values(signals, |mut row, signal| {
            row.push_bind(signal.detector.as_str())
                .push_bind(signal.protocol.as_str())
                .push_bind(signal.pool_address.to_string())
                .push_bind(signal.severity.as_str())
                .push_bind(signal.value)
                .push_bind(signal.threshold)
                .push_bind(signal.message.as_deref())
                .push_bind(signal.triggered_at);
        });

        builder
            .build()
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn latest_severity_by_pool(
        &self,
        detector: &str,
        since: DateTime<Utc>,
    ) -> RepositoryResult<HashMap<Pubkey, Severity>> {
        // The most recent row per pool: dedup only escalates within a window, so
        // its severity is also the max.
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT ON (pool_address)
                pool_address AS "pool_address!",
                severity     AS "severity!"
            FROM signals
            WHERE detector = $1 AND triggered_at > $2
            ORDER BY pool_address, triggered_at DESC
            "#,
            detector,
            since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let mut out = HashMap::with_capacity(rows.len());
        for row in rows {
            let pool = convert_string_to_pubkey(row.pool_address, "pool_address")?;
            let severity = Severity::from_str(&row.severity).map_err(|_| {
                RepositoryError::Integrity(format!("invalid severity `{}`", row.severity))
            })?;
            out.insert(pool, severity);
        }
        Ok(out)
    }
}

#[async_trait]
impl SignalFeed for PgSignalRepository {
    /// Newest first (`triggered_at DESC, id DESC`). The optional filters stay static SQL
    /// (`$n IS NULL OR <col> = $n`), so `query_as!` still checks them. A backward query
    /// reverses the ORDER BY, then the vector, to return the page in display order.
    async fn list(
        &self,
        filter: SignalFilter,
        cursor: Option<SignalCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<SignalRecord>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1;

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_triggered_at, cursor_id) = match active_cursor {
            Some(c) => (Some(c.triggered_at), Some(c.id)),
            None => (None, None),
        };
        let severity_filter = filter.severity.map(|s| s.as_str().to_string());
        let protocol_filter = filter.protocol.map(|p| p.as_str());
        let pool_filter = filter.pool.map(|p| p.to_string());

        let rows: Vec<SignalRow> = match mode {
            QueryMode::Forward => sqlx::query_as!(
                SignalRow,
                r#"
                SELECT id, detector, protocol, pool_address,
                       severity, value, threshold, message,
                       triggered_at
                FROM signals
                WHERE ($1::TEXT IS NULL OR severity = $1)
                  AND ($2::TEXT IS NULL OR pool_address = $2)
                  AND ($6::TEXT IS NULL OR protocol = $6)
                  AND (
                      $3::TIMESTAMPTZ IS NULL
                      OR triggered_at < $3
                      OR (triggered_at = $3 AND id < $4)
                  )
                ORDER BY triggered_at DESC, id DESC
                LIMIT $5
                "#,
                severity_filter,
                pool_filter,
                cursor_triggered_at,
                cursor_id,
                fetch_limit,
                protocol_filter,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,

            QueryMode::Backward => sqlx::query_as!(
                SignalRow,
                r#"
                SELECT id, detector, protocol, pool_address,
                       severity, value, threshold, message,
                       triggered_at
                FROM signals
                WHERE ($1::TEXT IS NULL OR severity = $1)
                  AND ($2::TEXT IS NULL OR pool_address = $2)
                  AND ($6::TEXT IS NULL OR protocol = $6)
                  AND (
                      $3::TIMESTAMPTZ IS NULL
                      OR triggered_at > $3
                      OR (triggered_at = $3 AND id > $4)
                  )
                ORDER BY triggered_at ASC, id ASC
                LIMIT $5
                "#,
                severity_filter,
                pool_filter,
                cursor_triggered_at,
                cursor_id,
                fetch_limit,
                protocol_filter,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
        };

        let records: Vec<SignalRecord> = rows
            .into_iter()
            .map(SignalRecord::try_from)
            .collect::<Result<_, _>>()?;

        Ok(
            PageBuilder::new(records, effective_limit, mode, had_cursor).finalize(|r| {
                Cursor::Signal(SignalCursor {
                    triggered_at: r.signal.triggered_at,
                    id: r.id,
                })
            }),
        )
    }

    async fn latest_cursor(&self) -> RepositoryResult<Option<SignalCursor>> {
        let row = sqlx::query!(
            r#"
            SELECT triggered_at AS "triggered_at!", id AS "id!"
            FROM signals
            ORDER BY triggered_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.map(|r| SignalCursor {
            triggered_at: r.triggered_at,
            id: r.id,
        }))
    }

    async fn newer_than(
        &self,
        after: &SignalCursor,
        limit: i64,
    ) -> RepositoryResult<Vec<SignalRecord>> {
        let capped = limit.clamp(1, MAX_PAGE_SIZE);

        // ASC: a stream delivers chronologically, unlike the feed's DESC display.
        let rows: Vec<SignalRow> = sqlx::query_as!(
            SignalRow,
            r#"
            SELECT id, detector, protocol, pool_address,
                   severity, value, threshold, message,
                   triggered_at
            FROM signals
            WHERE triggered_at > $1
               OR (triggered_at = $1 AND id > $2)
            ORDER BY triggered_at ASC, id ASC
            LIMIT $3
            "#,
            after.triggered_at,
            after.id,
            capped,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(SignalRecord::try_from).collect()
    }
}
