//! PostgreSQL implementation of [`RotationRunLookup`]: the rotation runs yog-analytic drew,
//! listed and summed through the last tranche written, and their volume by frozen hour.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use rows::{RotationRunRow, RotationRunTotalsRow, RunSplitRow};
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        FrozenPeriod, Protocol, ProtocolBucket, RotationRun, RotationRunCursor, RotationRunLookup,
        RotationRunOrder, RotationRunScope, RotationRunTotals, RunSplit, TRANCHE, TranchePeriod,
        Window,
    },
};

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, interval_of, map_sqlx_error,
};

pub struct PgRotationRunRepository {
    pool: PgPool,
}

impl PgRotationRunRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RotationRunLookup for PgRotationRunRepository {
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>> {
        match protocol {
            // ⚠️ The tranche ledger is not per protocol: only DAMM v2's writer measures rotation.
            Protocol::MeteoraDammV2 => {
                let row = sqlx::query!(
                    r#"
                    SELECT MIN(bucket) AS "since?: DateTime<Utc>"
                      FROM analytic_tranches
                     WHERE rotations
                    "#,
                )
                .fetch_one(&self.pool)
                .await
                .map_err(map_sqlx_error)?;
                Ok(row.since)
            }
            Protocol::MeteoraDlmm => Ok(None),
        }
    }

    async fn find_written_through(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        let row = sqlx::query!(
            r#"
            SELECT MAX(bucket) AS "last?: DateTime<Utc>"
              FROM analytic_tranches
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(row.last.map(|last| last + TRANCHE))
    }

    async fn find_totals(
        &self,
        period: &TranchePeriod,
        protocol: Protocol,
    ) -> RepositoryResult<RotationRunTotals> {
        let row = sqlx::query_as!(
            RotationRunTotalsRow,
            r#"
            SELECT COUNT(*) AS "runs!", COUNT(DISTINCT pool_address) AS "pools!",
                   COUNT(*) FILTER (WHERE NOT extinct) AS "in_progress!",
                   COALESCE(SUM(volume_usd), 0) AS "volume_usd!",
                   -- ⚠️ Until its first hour ends before the period does, a run's volume is all in it.
                   COUNT(*) FILTER (WHERE started_at + INTERVAL '1 hour' < $2)
                       AS "first_hour_runs!",
                   SUM(volume_usd) FILTER (WHERE started_at + INTERVAL '1 hour' < $2)
                       AS "first_hour_runs_volume_usd?",
                   SUM(first_hour_volume_usd) FILTER (WHERE started_at + INTERVAL '1 hour' < $2)
                       AS "in_first_hour_usd?",
                   ROUND(percentile_cont(0.5) WITHIN GROUP (
                       ORDER BY EXTRACT(EPOCH FROM ended_at - started_at)))::BIGINT
                       AS "median_duration_seconds?",
                   ROUND(percentile_cont(0.9) WITHIN GROUP (
                       ORDER BY EXTRACT(EPOCH FROM ended_at - started_at)))::BIGINT
                       AS "p90_duration_seconds?"
              FROM analytic_rotation_runs
             WHERE protocol = $3 AND started_at >= $1 AND started_at < $2
            "#,
            period.from,
            period.as_of,
            protocol.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        RotationRunTotals::try_from(row)
    }

    async fn list_runs(
        &self,
        period: &TranchePeriod,
        window: Window,
        scope: RotationRunScope,
        order: RotationRunOrder,
        cursor: Option<RotationRunCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<RotationRun>> {
        let had_cursor = cursor.is_some();
        let (after_volume, after_started, after_pool) = match &cursor {
            Some(c) => (
                Some(convert_decimal_to_bigdecimal(
                    c.volume_usd,
                    "cursor volume_usd",
                )?),
                Some(c.started_at),
                Some(c.pool_address.to_string()),
            ),
            None => (None, None, None),
        };
        // ⚠️ Ranked before it is named: joined first, a page costs its whole window.
        let rows = match (scope, order) {
            (RotationRunScope::Protocol(protocol), RotationRunOrder::Volume) => {
                let key = protocol.as_str();
                sqlx::query_as!(
                    RotationRunRow,
                    r#"
                SELECT r.protocol, r.pool_address, r.started_at, r.ended_at, r.volume_usd,
                       r.first_hour_volume_usd, r.extinct, ROUND(r.volume_usd, 6) AS "volume_key!",
                       ta.symbol AS "token_a_symbol?", tb.symbol AS "token_b_symbol?"
                  FROM (
                        SELECT r.*
                          FROM analytic_rotation_runs r
                         WHERE r.protocol = $3 AND r.started_at >= $1 AND r.started_at < $2
                           -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits.
                           AND ($4::NUMERIC IS NULL
                                OR ROUND(r.volume_usd, 6) < $4
                                OR (ROUND(r.volume_usd, 6) = $4 AND (r.pool_address > $6
                                    OR (r.pool_address = $6 AND r.started_at > $5))))
                         ORDER BY ROUND(r.volume_usd, 6) DESC, r.pool_address, r.started_at
                         LIMIT $7
                  ) r
                  LEFT JOIN pools p ON p.pool_address = r.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 ORDER BY ROUND(r.volume_usd, 6) DESC, r.pool_address, r.started_at
                "#,
                    period.from,
                    period.as_of,
                    key,
                    after_volume,
                    after_started,
                    after_pool,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_error)?
            }
            (RotationRunScope::Protocol(protocol), RotationRunOrder::At) => {
                let key = protocol.as_str();
                sqlx::query_as!(
                    RotationRunRow,
                    r#"
                SELECT r.protocol, r.pool_address, r.started_at, r.ended_at, r.volume_usd,
                       r.first_hour_volume_usd, r.extinct, ROUND(r.volume_usd, 6) AS "volume_key!",
                       ta.symbol AS "token_a_symbol?", tb.symbol AS "token_b_symbol?"
                  FROM (
                        SELECT r.*
                          FROM analytic_rotation_runs r
                         WHERE r.protocol = $3 AND r.started_at >= $1 AND r.started_at < $2
                           AND ($4::TIMESTAMPTZ IS NULL
                                OR r.started_at < $4
                                OR (r.started_at = $4 AND r.pool_address > $5))
                         ORDER BY r.started_at DESC, r.pool_address
                         LIMIT $6
                  ) r
                  LEFT JOIN pools p ON p.pool_address = r.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 ORDER BY r.started_at DESC, r.pool_address
                "#,
                    period.from,
                    period.as_of,
                    key,
                    after_started,
                    after_pool,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_error)?
            }
            (RotationRunScope::Pool(pool), RotationRunOrder::Volume) => {
                let key = pool.to_string();
                sqlx::query_as!(
                    RotationRunRow,
                    r#"
                SELECT r.protocol, r.pool_address, r.started_at, r.ended_at, r.volume_usd,
                       r.first_hour_volume_usd, r.extinct, ROUND(r.volume_usd, 6) AS "volume_key!",
                       ta.symbol AS "token_a_symbol?", tb.symbol AS "token_b_symbol?"
                  FROM (
                        SELECT r.*
                          FROM analytic_rotation_runs r
                         WHERE r.pool_address = $3 AND r.started_at >= $1 AND r.started_at < $2
                           -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits.
                           AND ($4::NUMERIC IS NULL
                                OR ROUND(r.volume_usd, 6) < $4
                                OR (ROUND(r.volume_usd, 6) = $4 AND (r.pool_address > $6
                                    OR (r.pool_address = $6 AND r.started_at > $5))))
                         ORDER BY ROUND(r.volume_usd, 6) DESC, r.pool_address, r.started_at
                         LIMIT $7
                  ) r
                  LEFT JOIN pools p ON p.pool_address = r.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 ORDER BY ROUND(r.volume_usd, 6) DESC, r.pool_address, r.started_at
                "#,
                    period.from,
                    period.as_of,
                    key,
                    after_volume,
                    after_started,
                    after_pool,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_error)?
            }
            (RotationRunScope::Pool(pool), RotationRunOrder::At) => {
                let key = pool.to_string();
                sqlx::query_as!(
                    RotationRunRow,
                    r#"
                SELECT r.protocol, r.pool_address, r.started_at, r.ended_at, r.volume_usd,
                       r.first_hour_volume_usd, r.extinct, ROUND(r.volume_usd, 6) AS "volume_key!",
                       ta.symbol AS "token_a_symbol?", tb.symbol AS "token_b_symbol?"
                  FROM (
                        SELECT r.*
                          FROM analytic_rotation_runs r
                         WHERE r.pool_address = $3 AND r.started_at >= $1 AND r.started_at < $2
                           AND ($4::TIMESTAMPTZ IS NULL
                                OR r.started_at < $4
                                OR (r.started_at = $4 AND r.pool_address > $5))
                         ORDER BY r.started_at DESC, r.pool_address
                         LIMIT $6
                  ) r
                  LEFT JOIN pools p ON p.pool_address = r.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 ORDER BY r.started_at DESC, r.pool_address
                "#,
                    period.from,
                    period.as_of,
                    key,
                    after_started,
                    after_pool,
                    limit + 1,
                )
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_error)?
            }
        };
        let runs = rows
            .into_iter()
            .map(RotationRunRow::into_keyed)
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(runs, limit, QueryMode::Forward, had_cursor).finalize(|(run, key)| {
                Cursor::RotationRun(Box::new(RotationRunCursor {
                    scope,
                    window,
                    as_of: period.as_of,
                    order,
                    volume_usd: *key,
                    started_at: run.started_at,
                    pool_address: run.pool_address,
                }))
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            items: page.items.into_iter().map(|(run, _)| run).collect(),
            next_cursor: page.next_cursor,
            prev_cursor: None,
            is_first: page.is_first,
            is_last: page.is_last,
        })
    }

    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        runs_since: Option<DateTime<Utc>>,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>> {
        let rows = sqlx::query_as!(
            RunSplitRow,
            r#"
            SELECT $4::TEXT AS "protocol!", h.bucket AS "bucket_from!", h.volume_usd,
                   CASE WHEN h.bucket >= $5 AND h.volume_usd IS NOT NULL
                        THEN COALESCE(r.volume_usd, 0) END AS in_runs_usd
              FROM analytic_protocol_hourly h
              LEFT JOIN (
                  SELECT bucket, SUM(volume_usd) AS volume_usd
                    FROM analytic_rotation_run_hours
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
                   GROUP BY bucket
              ) r ON r.bucket = h.bucket
             WHERE h.protocol = $4
               AND h.bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND h.bucket < $2
             ORDER BY h.bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
            runs_since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        runs_since: Option<DateTime<Utc>>,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>> {
        // ⚠️ The runs summed by hour first: several in one hour would repeat its volume in the day.
        let rows = sqlx::query_as!(
            RunSplitRow,
            r#"
            SELECT $4::TEXT AS "protocol!", h.bucket_from AS "bucket_from!",
                   SUM(h.volume_usd) AS volume_usd,
                   CASE WHEN h.bucket_from >= $5 AND COUNT(h.volume_usd) = COUNT(*)
                        THEN COALESCE(SUM(r.volume_usd), 0) END
                       AS in_runs_usd
              FROM (
                  SELECT bucket, volume_usd,
                         time_bucket($6::INTERVAL, bucket, origin => $2::TIMESTAMPTZ)
                             AS bucket_from
                    FROM analytic_protocol_hourly
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
              ) h
              LEFT JOIN (
                  SELECT bucket, SUM(volume_usd) AS volume_usd
                    FROM analytic_rotation_run_hours
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
                   GROUP BY bucket
              ) r ON r.bucket = h.bucket
             WHERE h.bucket_from >= $3
             GROUP BY h.bucket_from
             ORDER BY h.bucket_from
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
            runs_since,
            interval_of(width),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_tranches(
        &self,
        period: &TranchePeriod,
        runs_since: Option<DateTime<Utc>>,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<RunSplit>>> {
        // ⚠️ A run's tranches read pool by pool through the key: joined whole, every pool is read.
        let rows = sqlx::query_as!(
            RunSplitRow,
            r#"
            SELECT $4::TEXT AS "protocol!", t.bucket AS "bucket_from!",
                   CASE WHEN v.bucket IS NULL THEN 0 ELSE v.volume_usd END AS volume_usd,
                   CASE WHEN t.bucket >= $5 AND (v.bucket IS NULL OR v.volume_usd IS NOT NULL)
                        THEN COALESCE(r.volume_usd, 0) END AS in_runs_usd
              FROM generate_series(GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ),
                                   $2::TIMESTAMPTZ - INTERVAL '5 minutes',
                                   INTERVAL '5 minutes') AS t (bucket)
              LEFT JOIN (
                  SELECT bucket, SUM(volume_in_a_usd + volume_in_b_usd) AS volume_usd
                    FROM analytic_pool_5min
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
                   GROUP BY bucket
              ) v ON v.bucket = t.bucket
              LEFT JOIN (
                  SELECT f.bucket, SUM(f.volume_in_a_usd + f.volume_in_b_usd) AS volume_usd
                    FROM analytic_rotation_runs run
                   CROSS JOIN LATERAL (
                       SELECT bucket, volume_in_a_usd, volume_in_b_usd
                         FROM analytic_pool_5min
                        WHERE pool_address = run.pool_address AND valued
                          AND bucket >= GREATEST(run.started_at, $1::TIMESTAMPTZ)
                          AND bucket < LEAST(run.ended_at, $2::TIMESTAMPTZ)
                   ) f
                   WHERE run.protocol = $4 AND run.started_at < $2 AND run.ended_at > $1
                   GROUP BY f.bucket
              ) r ON r.bucket = t.bucket
             ORDER BY t.bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
            runs_since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }
}
