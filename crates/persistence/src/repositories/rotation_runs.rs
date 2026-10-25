//! PostgreSQL implementation of [`RotationRunLookup`]: the rotation runs yog-analytic drew,
//! listed and summed through the last tranche written, and their volume by frozen hour.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{RotationRunHourRow, RotationRunRow, RotationRunTotalsRow};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        FrozenPeriod, Protocol, ProtocolBucket, RotationRun, RotationRunCursor, RotationRunLookup,
        RotationRunOrder, RotationRunScope, RotationRunTotals, TRANCHE, TranchePeriod, Window,
    },
};

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, map_sqlx_error,
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
            SELECT COUNT(*) AS "runs!",
                   COALESCE(SUM(volume_usd), 0) AS "volume_usd!",
                   COALESCE(SUM(first_hour_volume_usd), 0) AS "first_hour_volume_usd!"
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
        let rows = match (scope, order) {
            (RotationRunScope::Protocol(protocol), RotationRunOrder::Volume) => {
                let key = protocol.as_str();
                sqlx::query_as!(
                    RotationRunRow,
                    r#"
                SELECT protocol, pool_address, started_at, ended_at, volume_usd,
                       first_hour_volume_usd, extinct, ROUND(volume_usd, 6) AS "volume_key!"
                  FROM analytic_rotation_runs
                 WHERE protocol = $3 AND started_at >= $1 AND started_at < $2
                   -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits.
                   AND ($4::NUMERIC IS NULL
                        OR ROUND(volume_usd, 6) < $4
                        OR (ROUND(volume_usd, 6) = $4 AND (pool_address > $6
                            OR (pool_address = $6 AND started_at > $5))))
                 ORDER BY ROUND(volume_usd, 6) DESC, pool_address, started_at
                 LIMIT $7
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
                SELECT protocol, pool_address, started_at, ended_at, volume_usd,
                       first_hour_volume_usd, extinct, ROUND(volume_usd, 6) AS "volume_key!"
                  FROM analytic_rotation_runs
                 WHERE protocol = $3 AND started_at >= $1 AND started_at < $2
                   AND ($4::TIMESTAMPTZ IS NULL
                        OR started_at < $4
                        OR (started_at = $4 AND pool_address > $5))
                 ORDER BY started_at DESC, pool_address
                 LIMIT $6
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
                SELECT protocol, pool_address, started_at, ended_at, volume_usd,
                       first_hour_volume_usd, extinct, ROUND(volume_usd, 6) AS "volume_key!"
                  FROM analytic_rotation_runs
                 WHERE pool_address = $3 AND started_at >= $1 AND started_at < $2
                   -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits.
                   AND ($4::NUMERIC IS NULL
                        OR ROUND(volume_usd, 6) < $4
                        OR (ROUND(volume_usd, 6) = $4 AND (pool_address > $6
                            OR (pool_address = $6 AND started_at > $5))))
                 ORDER BY ROUND(volume_usd, 6) DESC, pool_address, started_at
                 LIMIT $7
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
                SELECT protocol, pool_address, started_at, ended_at, volume_usd,
                       first_hour_volume_usd, extinct, ROUND(volume_usd, 6) AS "volume_key!"
                  FROM analytic_rotation_runs
                 WHERE pool_address = $3 AND started_at >= $1 AND started_at < $2
                   AND ($4::TIMESTAMPTZ IS NULL
                        OR started_at < $4
                        OR (started_at = $4 AND pool_address > $5))
                 ORDER BY started_at DESC, pool_address
                 LIMIT $6
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
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<Decimal>>> {
        let rows = sqlx::query_as!(
            RotationRunHourRow,
            r#"
            SELECT protocol AS "protocol!", bucket AS "bucket_from!",
                   SUM(volume_usd) AS "volume_usd!"
              FROM analytic_rotation_run_hours
             WHERE protocol = $4
               AND bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
             GROUP BY protocol, bucket
             ORDER BY bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<Decimal>>> {
        let rows = sqlx::query_as!(
            RotationRunHourRow,
            r#"
            SELECT protocol AS "protocol!", bucket_from AS "bucket_from!",
                   SUM(volume_usd) AS "volume_usd!"
              FROM (
                  SELECT h.protocol, h.volume_usd,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bucket_from
                    FROM analytic_rotation_run_hours h
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
              ) hours
             WHERE bucket_from >= $3
             GROUP BY protocol, bucket_from
             ORDER BY bucket_from
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }
}
