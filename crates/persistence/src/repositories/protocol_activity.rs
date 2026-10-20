//! PostgreSQL implementation of [`ProtocolActivityLookup`]: yog-analytic's
//! tables by protocol, read over the span the caller built. Summing the hours
//! of a span is exact for every flow; the distinct pools are read as written.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{ProtocolActivityBarRow, ProtocolActivityRow, ProtocolTvlRow};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        FrozenHours, ProtocolActivity, ProtocolActivityBar, ProtocolActivityLookup, ProtocolTvl,
        Span, Window,
    },
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgProtocolActivityRepository {
    pool: PgPool,
}

impl PgProtocolActivityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProtocolActivityLookup for PgProtocolActivityRepository {
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>> {
        let row = sqlx::query!(
            r#"
            SELECT MIN(bucket) AS "first?: DateTime<Utc>",
                   MAX(bucket) FILTER (WHERE frozen_at IS NOT NULL)
                       AS "last_frozen?: DateTime<Utc>"
              FROM analytic_hours
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row
            .first
            .zip(row.last_frozen)
            .map(|(first, last_frozen)| FrozenHours { first, last_frozen }))
    }

    async fn activity(
        &self,
        span: &Span,
        window: Window,
    ) -> RepositoryResult<Vec<ProtocolActivity>> {
        let rows = sqlx::query_as!(
            ProtocolActivityRow,
            r#"
            WITH facts AS (
                SELECT protocol,
                       SUM(volume_usd) AS volume_usd, SUM(fees_usd) AS fees_usd,
                       SUM(lp_fees_usd) AS lp_fees_usd,
                       SUM(protocol_fees_usd) AS protocol_fees_usd,
                       SUM(referral_fees_usd) AS referral_fees_usd,
                       SUM(swap_count)::BIGINT AS swap_count,
                       SUM(active_pools)::BIGINT AS pool_hours,
                       SUM(active_pools_valued)::BIGINT AS pool_hours_valued
                  FROM analytic_protocol_hourly
                 WHERE bucket >= $1 AND bucket < $2
                 GROUP BY protocol
            ),
            births AS (
                SELECT protocol, SUM(births)::BIGINT AS births
                  FROM analytic_protocol_births_hourly
                 WHERE bucket >= $1 AND bucket < $2
                 GROUP BY protocol
            ),
            -- ⚠️ A full join: an hour of births without a swap still counts.
            joined AS (
                SELECT COALESCE(f.protocol, b.protocol) AS protocol,
                       f.volume_usd, f.fees_usd, f.lp_fees_usd,
                       f.protocol_fees_usd, f.referral_fees_usd,
                       f.swap_count, f.pool_hours, f.pool_hours_valued, b.births
                  FROM facts f
                  FULL JOIN births b ON b.protocol = f.protocol
            )
            SELECT j.protocol AS "protocol!",
                   j.volume_usd, j.fees_usd, j.lp_fees_usd,
                   j.protocol_fees_usd, j.referral_fees_usd,
                   COALESCE(j.swap_count, 0) AS "swap_count!",
                   p.active_pools AS "active_pools?",
                   COALESCE(j.births, 0) AS "births!",
                   COALESCE(j.pool_hours, 0) AS "pool_hours!",
                   COALESCE(j.pool_hours_valued, 0) AS "pool_hours_valued!"
              FROM joined j
              LEFT JOIN analytic_protocol_periods p
                     ON p.protocol = j.protocol
                    AND p.period = $3
                    AND p.bucket = $4
             ORDER BY j.protocol
            "#,
            span.from,
            span.as_of,
            window.as_str(),
            span.last_hour(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolActivity::try_from).collect()
    }

    async fn activity_hourly(&self, span: &Span) -> RepositoryResult<Vec<ProtocolActivityBar>> {
        let rows = sqlx::query_as!(
            ProtocolActivityBarRow,
            r#"
            SELECT protocol, bucket AS bar_from, volume_usd, fees_usd, swap_count,
                   active_pools AS pool_hours, active_pools_valued AS pool_hours_valued
              FROM analytic_protocol_hourly
             WHERE bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
             ORDER BY bucket, protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolActivityBar::try_from)
            .collect()
    }

    async fn activity_daily(&self, span: &Span) -> RepositoryResult<Vec<ProtocolActivityBar>> {
        // ⚠️ Days aligned on `as_of`, not on midnight: the bars then add up to the span.
        let rows = sqlx::query_as!(
            ProtocolActivityBarRow,
            r#"
            SELECT protocol, bar_from AS "bar_from!",
                   SUM(volume_usd) AS volume_usd, SUM(fees_usd) AS fees_usd,
                   SUM(swap_count)::BIGINT AS "swap_count!",
                   SUM(active_pools)::BIGINT AS "pool_hours!",
                   SUM(active_pools_valued)::BIGINT AS "pool_hours_valued!"
              FROM (
                  SELECT h.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bar_from
                    FROM analytic_protocol_hourly h
                   WHERE bucket >= $1 AND bucket < $2
              ) hours
             WHERE bar_from >= $3
             GROUP BY protocol, bar_from
             ORDER BY bar_from, protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolActivityBar::try_from)
            .collect()
    }

    async fn tvl_latest(&self, last: DateTime<Utc>) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT DISTINCT ON (protocol)
                   protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced
              FROM analytic_protocol_tvl
             WHERE bucket <= $1
             ORDER BY protocol, bucket DESC
            "#,
            last,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }

    async fn tvl_hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced
              FROM analytic_protocol_tvl
             WHERE bucket BETWEEN $1 AND $2
             ORDER BY bucket, protocol
            "#,
            first,
            last,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }
}
