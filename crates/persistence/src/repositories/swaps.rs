//! PostgreSQL implementation of [`SwapLookup`]: yog-analytic's protocol and pool hours, read
//! over the period the caller built.

mod rows;

use async_trait::async_trait;
use rows::{PoolSwapBucketRow, ProtocolSwapActivityRow, ProtocolSwapBucketRow, SwapActivityRow};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        FrozenPeriod, PoolSwapBucket, Protocol, ProtocolBucket, ProtocolSwapActivity, SwapActivity,
        SwapLookup, Window,
    },
};

use crate::repositories::helper::{bind_protocol_filter, map_sqlx_error};

pub struct PgSwapRepository {
    pool: PgPool,
}

impl PgSwapRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SwapLookup for PgSwapRepository {
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

    async fn list_protocol_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBucket<SwapActivity>>> {
        // ⚠️ Days aligned on `as_of`, not on midnight: the buckets then add up to the period.
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
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
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
            SELECT SUM(swap_count)::BIGINT AS "swaps!",
                   SUM(volume_in_a_usd + volume_in_b_usd) AS volume_usd,
                   SUM(fees_usd) AS fees_usd, SUM(lp_fees_usd) AS lp_fees_usd,
                   SUM(protocol_fees_usd) AS protocol_fees_usd,
                   SUM(referral_fees_usd) AS referral_fees_usd,
                   COUNT(*) AS "pool_hours!",
                   COUNT(*) FILTER (WHERE valued) AS "pool_hours_valued!"
              FROM analytic_pool_hourly
             WHERE pool_address = $3 AND bucket >= $1 AND bucket < $2
            HAVING COUNT(*) > 0
            "#,
            period.from,
            period.as_of,
            pool.to_string(),
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
            SELECT bucket AS bucket_from, swap_count AS swaps,
                   volume_in_a_usd + volume_in_b_usd AS volume_usd,
                   fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd,
                   1::BIGINT AS "pool_hours!",
                   (CASE WHEN valued THEN 1 ELSE 0 END)::BIGINT AS "pool_hours_valued!"
              FROM analytic_pool_hourly
             WHERE pool_address = $4
               AND bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
             ORDER BY bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            pool.to_string(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBucket::try_from).collect()
    }

    async fn list_pool_daily(
        &self,
        period: &FrozenPeriod,
        pool: Pubkey,
    ) -> RepositoryResult<Vec<PoolSwapBucket>> {
        let rows = sqlx::query_as!(
            PoolSwapBucketRow,
            r#"
            SELECT bucket_from AS "bucket_from!",
                   SUM(swap_count)::BIGINT AS "swaps!",
                   SUM(volume_in_a_usd + volume_in_b_usd) AS volume_usd,
                   SUM(fees_usd) AS fees_usd, SUM(lp_fees_usd) AS lp_fees_usd,
                   SUM(protocol_fees_usd) AS protocol_fees_usd,
                   SUM(referral_fees_usd) AS referral_fees_usd,
                   COUNT(*) AS "pool_hours!",
                   COUNT(*) FILTER (WHERE valued) AS "pool_hours_valued!"
              FROM (
                  SELECT h.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bucket_from
                    FROM analytic_pool_hourly h
                   WHERE pool_address = $4 AND bucket >= $1 AND bucket < $2
              ) hours
             WHERE bucket_from >= $3
             GROUP BY bucket_from
             ORDER BY bucket_from
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            pool.to_string(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBucket::try_from).collect()
    }
}
