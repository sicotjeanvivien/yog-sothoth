//! Postgres implementation of [`SignalRepository`] and
//! [`SignalFeed`] — one struct, two consumer lenses.
//!
//! Backed by the `signals` hypertable (migration 022). The engine's
//! contract is append-only: a plain multi-row INSERT (signals are
//! immutable conclusions, no `ON CONFLICT` / UPSERT path) plus the dedup
//! read. The api's feed contract paginates with the same bidirectional
//! keyset machinery as the swap/liquidity event repositories (static
//! SQL, one query per traversal mode).
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
    RepositoryError, RepositoryResult,
    domain::{Severity, Signal, SignalCursor, SignalFeed, SignalRecord, SignalRepository},
    tools::{Cursor, Page, PageDirection, PagePosition},
};

/// Maximum number of rows returned in a single page, regardless of the
/// caller's `limit`. Backstop against an oversized query if the
/// API-layer validation is bypassed.
const MAX_PAGE_SIZE: i64 = 200;

/// Postgres-backed signal repository.
#[derive(Clone)]
pub struct PgSignalRepository {
    pool: PgPool,
}

impl PgSignalRepository {
    /// Build the repository over a shared connection pool.
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

        // Variable-arity bulk insert: QueryBuilder, since the `query!` macros
        // cannot generate a runtime-sized VALUES list (same reasoning as the
        // token_prices batch insert).
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
        // DISTINCT ON keeps the most recent row per pool. Because the engine's
        // dedup only ever emits an escalating severity within a window, the
        // most recent severity is also the max — the current suppression state.
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
    /// Paginate the signal feed with bidirectional navigation.
    ///
    /// Natural display order is `triggered_at DESC, id DESC` (newest
    /// first, deterministic tie-break on the storage id).
    ///
    /// - `severity` and `pool` are optional exact filters, folded into
    ///   the static SQL as `$n IS NULL OR <col> = $n` — optional
    ///   equalities are not a dynamic query shape, so the `query_as!`
    ///   compile check is kept.
    /// - `cursor` + `direction` cooperate: traverse forward (older
    ///   signals) or backward (newer signals) from the cursor position.
    /// - `position` jumps to a list boundary (`First` = newest, `Last` =
    ///   oldest), ignoring any cursor.
    /// - Peek N+1 detects whether more rows exist on the queried side in
    ///   a single query.
    ///
    /// Backward queries (Prev / Last) reverse the SQL ORDER BY and the
    /// resulting vector before returning, so the caller always observes
    /// the page in display order.
    async fn list(
        &self,
        severity: Option<Severity>,
        pool: Option<Pubkey>,
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
        let severity_filter = severity.map(|s| s.as_str().to_string());
        let pool_filter = pool.map(|p| p.to_string());

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

        // Strictly-after keyset, ASC: the delivery order of a stream is
        // chronological, unlike the feed's DESC display order.
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

    async fn recent_by_pools(
        &self,
        pools: &[Pubkey],
        since: DateTime<Utc>,
        per_pool_limit: i64,
    ) -> RepositoryResult<HashMap<Pubkey, Vec<SignalRecord>>> {
        if pools.is_empty() {
            return Ok(HashMap::new());
        }

        let addresses: Vec<String> = pools.iter().map(|p| p.to_string()).collect();
        let capped = per_pool_limit.clamp(1, MAX_PAGE_SIZE);

        // ROW_NUMBER caps the rows *per pool* in SQL, so one noisy pool
        // cannot bloat the whole page's payload. The (pool_address,
        // triggered_at DESC) index (migration 022) drives the window scan.
        // Columns come through a subquery, so sqlx sees them as nullable —
        // the `!` markers restore the base table's constraints.
        let rows: Vec<SignalRow> = sqlx::query_as!(
            SignalRow,
            r#"
            SELECT id           AS "id!",
                   detector     AS "detector!",
                   protocol     AS "protocol!",
                   pool_address AS "pool_address!",
                   severity     AS "severity!",
                   value        AS "value!",
                   threshold,
                   message,
                   triggered_at AS "triggered_at!"
            FROM (
                SELECT *, ROW_NUMBER() OVER (
                           PARTITION BY pool_address
                           ORDER BY triggered_at DESC, id DESC
                       ) AS rn
                FROM signals
                WHERE pool_address = ANY($1) AND triggered_at > $2
            ) ranked
            WHERE rn <= $3
            ORDER BY triggered_at DESC, id DESC
            "#,
            &addresses,
            since,
            capped,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        // Rows arrive newest-first globally; pushing in order keeps each
        // pool's vector newest-first too.
        let mut out: HashMap<Pubkey, Vec<SignalRecord>> = HashMap::new();
        for row in rows {
            let record = SignalRecord::try_from(row)?;
            out.entry(record.signal.pool_address)
                .or_default()
                .push(record);
        }
        Ok(out)
    }
}
