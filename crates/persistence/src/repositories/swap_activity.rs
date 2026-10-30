//! PostgreSQL implementation of [`SwapActivityLookup`]: yog-analytic's protocol and pool hours, read
//! over the period the caller built.

mod rows;

use async_trait::async_trait;
use chrono::TimeDelta;
use rows::{PoolSwapBucketRow, ProtocolSwapActivityRow, ProtocolSwapBucketRow, SwapActivityRow};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        FrozenPeriod, PoolSwapBucket, Protocol, ProtocolBucket, ProtocolSwapActivity, SwapActivity,
        SwapActivityLookup, TranchePeriod, Window,
    },
};

use crate::repositories::analytic::MEASURED;
use crate::repositories::helper::{bind_protocol_filter, interval_of, map_sqlx_error};

pub struct PgSwapActivityRepository {
    pool: PgPool,
}

impl PgSwapActivityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SwapActivityLookup for PgSwapActivityRepository {
    async fn find_protocol_totals(
        &self,
        period: &FrozenPeriod,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapActivity>> {
        let rows = sqlx::query_as!(
            ProtocolSwapActivityRow,
            r#"
            SELECT h.protocol AS "protocol!",
                   SUM(h.swap_count)::BIGINT AS "swaps!",
                   SUM(h.volume_usd) AS volume_usd, SUM(h.fees_usd) AS fees_usd,
                   SUM(h.lp_fees_usd) AS lp_fees_usd,
                   SUM(h.protocol_fees_usd) AS protocol_fees_usd,
                   SUM(h.referral_fees_usd) AS referral_fees_usd,
                   SUM(h.active_pools)::BIGINT AS "pool_hours!",
                   SUM(h.active_pools_valued)::BIGINT AS "pool_hours_valued!",
                   MAX(p.active_pools)::BIGINT AS pools
              FROM analytic_protocol_hourly h
              LEFT JOIN analytic_protocol_periods p
                     ON p.protocol = h.protocol AND p.period = $3 AND p.bucket = $4
             WHERE h.bucket >= $1 AND h.bucket < $2
               AND ($5::TEXT IS NULL OR h.protocol = $5)
             GROUP BY h.protocol
             ORDER BY h.protocol
            "#,
            period.from,
            period.as_of,
            window.as_str(),
            period.last_hour(),
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolSwapActivity::try_from)
            .collect()
    }

    async fn list_protocol_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        let rows = sqlx::query_as!(
            ProtocolSwapBucketRow,
            r#"
            SELECT protocol, bucket AS bucket_from, swap_count AS swaps,
                   volume_usd, fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd,
                   active_pools AS pool_hours, active_pools_valued AS pool_hours_valued
              FROM analytic_protocol_hourly
             WHERE bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
               AND ($4::TEXT IS NULL OR protocol = $4)
             ORDER BY bucket, protocol
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_protocol_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        // ⚠️ Aligned on `as_of`, not on midnight: the buckets then add up to the period.
        let rows = sqlx::query_as!(
            ProtocolSwapBucketRow,
            r#"
            SELECT protocol AS "protocol!", bucket_from AS "bucket_from!",
                   SUM(swap_count)::BIGINT AS "swaps!",
                   SUM(volume_usd) AS volume_usd, SUM(fees_usd) AS fees_usd,
                   SUM(lp_fees_usd) AS lp_fees_usd, SUM(protocol_fees_usd) AS protocol_fees_usd,
                   SUM(referral_fees_usd) AS referral_fees_usd,
                   SUM(active_pools)::BIGINT AS "pool_hours!",
                   SUM(active_pools_valued)::BIGINT AS "pool_hours_valued!"
              FROM (
                  SELECT h.*,
                         time_bucket($5::INTERVAL, bucket, origin => $2::TIMESTAMPTZ)
                             AS bucket_from
                    FROM analytic_protocol_hourly h
                   WHERE bucket >= $1 AND bucket < $2
                     AND ($4::TEXT IS NULL OR protocol = $4)
              ) hours
             WHERE bucket_from >= $3
             GROUP BY protocol, bucket_from
             ORDER BY bucket_from, protocol
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            bind_protocol_filter(protocol),
            interval_of(width),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn find_protocol_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapActivity>> {
        // ⚠️ 0 only for a protocol without any pool: one whose pools are all unvalued sums to NULL.
        let rows = sqlx::query_as!(
            ProtocolSwapActivityRow,
            r#"
            SELECT pr.protocol AS "protocol!",
                   COALESCE(SUM(f.swap_count), 0)::BIGINT AS "swaps!",
                   CASE WHEN COUNT(f.pool_address) = 0 THEN 0
                        ELSE SUM(f.volume_in_a_usd + f.volume_in_b_usd) END AS volume_usd,
                   CASE WHEN COUNT(f.pool_address) = 0 THEN 0 ELSE SUM(f.fees_usd) END AS fees_usd,
                   CASE WHEN COUNT(f.pool_address) = 0 THEN 0 ELSE SUM(f.lp_fees_usd) END
                       AS lp_fees_usd,
                   CASE WHEN COUNT(f.pool_address) = 0 THEN 0 ELSE SUM(f.protocol_fees_usd) END
                       AS protocol_fees_usd,
                   CASE WHEN COUNT(f.pool_address) = 0 THEN 0 ELSE SUM(f.referral_fees_usd) END
                       AS referral_fees_usd,
                   COUNT(f.pool_address) AS "pool_hours!",
                   COUNT(f.pool_address) FILTER (WHERE f.valued) AS "pool_hours_valued!",
                   COUNT(DISTINCT f.pool_address) AS pools
              FROM UNNEST($3::TEXT[]) AS pr (protocol)
              LEFT JOIN analytic_pool_5min f
                     ON f.protocol = pr.protocol AND f.bucket >= $1 AND f.bucket < $2
             WHERE $4::TEXT IS NULL OR pr.protocol = $4
             GROUP BY pr.protocol
             ORDER BY pr.protocol
            "#,
            period.from,
            period.as_of,
            &measured() as &[&str],
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolSwapActivity::try_from)
            .collect()
    }

    async fn list_protocol_tranches(
        &self,
        period: &TranchePeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        let rows = sqlx::query_as!(
            ProtocolSwapBucketRow,
            r#"
            SELECT pr.protocol AS "protocol!", t.bucket AS "bucket_from!",
                   COALESCE(a.swaps, 0)::BIGINT AS "swaps!",
                   CASE WHEN a.bucket IS NULL THEN 0 ELSE a.volume_usd END AS volume_usd,
                   CASE WHEN a.bucket IS NULL THEN 0 ELSE a.fees_usd END AS fees_usd,
                   CASE WHEN a.bucket IS NULL THEN 0 ELSE a.lp_fees_usd END AS lp_fees_usd,
                   CASE WHEN a.bucket IS NULL THEN 0 ELSE a.protocol_fees_usd END
                       AS protocol_fees_usd,
                   CASE WHEN a.bucket IS NULL THEN 0 ELSE a.referral_fees_usd END
                       AS referral_fees_usd,
                   COALESCE(a.pools, 0)::BIGINT AS "pool_hours!",
                   COALESCE(a.pools_valued, 0)::BIGINT AS "pool_hours_valued!"
              FROM generate_series(GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ),
                                   $2::TIMESTAMPTZ - INTERVAL '5 minutes',
                                   INTERVAL '5 minutes') AS t (bucket)
             CROSS JOIN UNNEST($4::TEXT[]) AS pr (protocol)
              LEFT JOIN (
                  SELECT protocol, bucket, SUM(swap_count) AS swaps,
                         SUM(volume_in_a_usd + volume_in_b_usd) AS volume_usd,
                         SUM(fees_usd) AS fees_usd, SUM(lp_fees_usd) AS lp_fees_usd,
                         SUM(protocol_fees_usd) AS protocol_fees_usd,
                         SUM(referral_fees_usd) AS referral_fees_usd,
                         COUNT(*) AS pools, COUNT(*) FILTER (WHERE valued) AS pools_valued
                    FROM analytic_pool_5min
                   WHERE bucket >= $1 AND bucket < $2
                   GROUP BY protocol, bucket
              ) a ON a.protocol = pr.protocol AND a.bucket = t.bucket
             WHERE $5::TEXT IS NULL OR pr.protocol = $5
             ORDER BY t.bucket, pr.protocol
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            &measured() as &[&str],
            bind_protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn find_pool_totals(
        &self,
        period: &FrozenPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Option<SwapActivity>> {
        let row = sqlx::query_as!(
            SwapActivityRow,
            r#"
            WITH facts AS MATERIALIZED (
                SELECT * FROM analytic_pool_hourly
                 WHERE pool_address = $3 AND bucket >= $1 AND bucket < $2
            ),
            seen AS (
                -- ⚠️ From its first fact when earlier: an event indexed late can precede `first_seen_at`.
                SELECT LEAST(date_trunc('hour', first_seen_at), (SELECT MIN(bucket) FROM facts))
                           AS since
                  FROM pools
                 WHERE pool_address = $3 AND protocol = ANY($4::TEXT[])
            )
            SELECT COALESCE(SUM(f.swap_count), 0)::BIGINT AS "swaps!",
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.volume_in_a_usd + f.volume_in_b_usd) END AS volume_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0 ELSE SUM(f.fees_usd) END AS fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0 ELSE SUM(f.lp_fees_usd) END AS lp_fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.protocol_fees_usd) END AS protocol_fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.referral_fees_usd) END AS referral_fees_usd,
                   COUNT(f.bucket) AS "pool_hours!",
                   COUNT(f.bucket) FILTER (WHERE f.valued) AS "pool_hours_valued!"
              FROM seen
              JOIN analytic_hours h ON h.bucket >= seen.since
              LEFT JOIN facts f ON f.bucket = h.bucket
             WHERE h.bucket >= $1 AND h.bucket < $2
            HAVING COUNT(*) > 0
            "#,
            period.from,
            period.as_of,
            pool.to_string(),
            &measured() as &[&str],
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(SwapActivity::try_from).transpose()
    }

    async fn list_pool_hourly(
        &self,
        period: &FrozenPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        let rows = sqlx::query_as!(
            PoolSwapBucketRow,
            r#"
            WITH facts AS MATERIALIZED (
                SELECT * FROM analytic_pool_hourly
                 WHERE pool_address = $4 AND bucket >= $1 AND bucket < $2
            ),
            seen AS (
                -- ⚠️ From its first fact when earlier: an event indexed late can precede `first_seen_at`.
                SELECT LEAST(date_trunc('hour', first_seen_at), (SELECT MIN(bucket) FROM facts))
                           AS since
                  FROM pools
                 WHERE pool_address = $4 AND protocol = ANY($5::TEXT[])
            )
            SELECT h.bucket AS bucket_from, COALESCE(f.swap_count, 0) AS "swaps!",
                   CASE WHEN f.bucket IS NULL THEN 0
                        ELSE f.volume_in_a_usd + f.volume_in_b_usd END AS volume_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.fees_usd END AS fees_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.lp_fees_usd END AS lp_fees_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.protocol_fees_usd END
                       AS protocol_fees_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.referral_fees_usd END
                       AS referral_fees_usd,
                   (CASE WHEN f.bucket IS NULL THEN 0 ELSE 1 END)::BIGINT AS "pool_hours!",
                   (CASE WHEN f.valued THEN 1 ELSE 0 END)::BIGINT AS "pool_hours_valued!"
              FROM seen
              JOIN analytic_hours h ON h.bucket >= seen.since
              LEFT JOIN facts f ON f.bucket = h.bucket
             WHERE h.bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND h.bucket < $2
             ORDER BY h.bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            pool.to_string(),
            &measured() as &[&str],
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBucket::try_from).collect()
    }

    async fn list_pool_buckets(
        &self,
        period: &FrozenPeriod,
        width: TimeDelta,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        let rows = sqlx::query_as!(
            PoolSwapBucketRow,
            r#"
            WITH facts AS MATERIALIZED (
                SELECT * FROM analytic_pool_hourly
                 WHERE pool_address = $4 AND bucket >= $1 AND bucket < $2
            ),
            seen AS (
                -- ⚠️ From its first fact when earlier: an event indexed late can precede `first_seen_at`.
                SELECT LEAST(date_trunc('hour', first_seen_at), (SELECT MIN(bucket) FROM facts))
                           AS since
                  FROM pools
                 WHERE pool_address = $4 AND protocol = ANY($5::TEXT[])
            )
            SELECT time_bucket($6::INTERVAL, h.bucket, origin => $2::TIMESTAMPTZ)
                       AS "bucket_from!",
                   COALESCE(SUM(f.swap_count), 0)::BIGINT AS "swaps!",
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.volume_in_a_usd + f.volume_in_b_usd) END AS volume_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0 ELSE SUM(f.fees_usd) END AS fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0 ELSE SUM(f.lp_fees_usd) END AS lp_fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.protocol_fees_usd) END AS protocol_fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.referral_fees_usd) END AS referral_fees_usd,
                   COUNT(f.bucket) AS "pool_hours!",
                   COUNT(f.bucket) FILTER (WHERE f.valued) AS "pool_hours_valued!"
              FROM seen
              JOIN analytic_hours h ON h.bucket >= seen.since
              LEFT JOIN facts f ON f.bucket = h.bucket
             WHERE h.bucket >= $1 AND h.bucket < $2
               AND time_bucket($6::INTERVAL, h.bucket, origin => $2::TIMESTAMPTZ) >= $3
             GROUP BY 1
             ORDER BY 1
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            pool.to_string(),
            &measured() as &[&str],
            interval_of(width),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBucket::try_from).collect()
    }
    async fn find_pool_totals_in_tranches(
        &self,
        period: &TranchePeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Option<SwapActivity>> {
        let row = sqlx::query_as!(
            SwapActivityRow,
            r#"
            WITH facts AS MATERIALIZED (
                SELECT * FROM analytic_pool_5min
                 WHERE pool_address = $3 AND bucket >= $1 AND bucket < $2
            ),
            seen AS (
                -- ⚠️ From its first fact when earlier: an event indexed late can precede `first_seen_at`.
                SELECT LEAST(time_bucket('5 minutes', first_seen_at), (SELECT MIN(bucket) FROM facts))
                           AS since
                  FROM pools
                 WHERE pool_address = $3 AND protocol = ANY($4::TEXT[])
            )
            SELECT COALESCE(SUM(f.swap_count), 0)::BIGINT AS "swaps!",
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.volume_in_a_usd + f.volume_in_b_usd) END AS volume_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0 ELSE SUM(f.fees_usd) END AS fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0 ELSE SUM(f.lp_fees_usd) END AS lp_fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.protocol_fees_usd) END AS protocol_fees_usd,
                   CASE WHEN COUNT(f.bucket) = 0 THEN 0
                        ELSE SUM(f.referral_fees_usd) END AS referral_fees_usd,
                   COUNT(f.bucket) AS "pool_hours!",
                   COUNT(f.bucket) FILTER (WHERE f.valued) AS "pool_hours_valued!"
              FROM seen
              JOIN analytic_tranches h ON h.bucket >= seen.since
              LEFT JOIN facts f ON f.bucket = h.bucket
             WHERE h.bucket >= $1 AND h.bucket < $2
            HAVING COUNT(*) > 0
            "#,
            period.from,
            period.as_of,
            pool.to_string(),
            &measured() as &[&str],
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(SwapActivity::try_from).transpose()
    }
    async fn list_pool_tranches(
        &self,
        period: &TranchePeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        let rows = sqlx::query_as!(
            PoolSwapBucketRow,
            r#"
            WITH facts AS MATERIALIZED (
                SELECT * FROM analytic_pool_5min
                 WHERE pool_address = $4 AND bucket >= $1 AND bucket < $2
            ),
            seen AS (
                -- ⚠️ From its first fact when earlier: an event indexed late can precede `first_seen_at`.
                SELECT LEAST(time_bucket('5 minutes', first_seen_at), (SELECT MIN(bucket) FROM facts))
                           AS since
                  FROM pools
                 WHERE pool_address = $4 AND protocol = ANY($5::TEXT[])
            )
            SELECT h.bucket AS bucket_from, COALESCE(f.swap_count, 0) AS "swaps!",
                   CASE WHEN f.bucket IS NULL THEN 0
                        ELSE f.volume_in_a_usd + f.volume_in_b_usd END AS volume_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.fees_usd END AS fees_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.lp_fees_usd END AS lp_fees_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.protocol_fees_usd END
                       AS protocol_fees_usd,
                   CASE WHEN f.bucket IS NULL THEN 0 ELSE f.referral_fees_usd END
                       AS referral_fees_usd,
                   (CASE WHEN f.bucket IS NULL THEN 0 ELSE 1 END)::BIGINT AS "pool_hours!",
                   (CASE WHEN f.valued THEN 1 ELSE 0 END)::BIGINT AS "pool_hours_valued!"
              FROM seen
              JOIN analytic_tranches h ON h.bucket >= seen.since
              LEFT JOIN facts f ON f.bucket = h.bucket
             WHERE h.bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND h.bucket < $2
             ORDER BY h.bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            pool.to_string(),
            &measured() as &[&str],
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBucket::try_from).collect()
    }
}

/// The protocols whose pools read 0 for a covered hour without a swap.
fn measured() -> Vec<&'static str> {
    MEASURED.iter().map(|p| p.as_str()).collect()
}
