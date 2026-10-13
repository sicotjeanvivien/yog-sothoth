//! PostgreSQL implementation of [`GlobalAnalyticsRepository`]: the last 24 h
//! of yog-analytic's protocol aggregate, and the last TVL it measured for each
//! protocol. The volume reads as NULL unless the hours written span the window
//! through the previous hour, the TVL unless measured since then: a stopped,
//! catching-up or new writer shows instead of publishing a partial sum.

mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::GlobalAnalyticsRow;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{GlobalAnalytics, GlobalAnalyticsRepository},
};

pub struct PgGlobalAnalyticsRepository {
    pool: PgPool,
}

impl PgGlobalAnalyticsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl GlobalAnalyticsRepository for PgGlobalAnalyticsRepository {
    async fn global_analytics(&self) -> RepositoryResult<GlobalAnalytics> {
        let row = sqlx::query_as!(
            GlobalAnalyticsRow,
            r#"
            WITH bound AS (
                SELECT date_trunc('hour', NOW(), 'UTC') - INTERVAL '23 hours' AS first_hour,
                       date_trunc('hour', NOW(), 'UTC') - INTERVAL '1 hour'   AS oldest
            ),
            written AS (
                SELECT MIN(bucket) AS first_hour, MAX(bucket) AS last_hour
                FROM analytic_hours
            ),
            tvl AS (
                SELECT
                    SUM(tvl_usd)                           AS total_tvl_usd,
                    COALESCE(SUM(pools_priced), 0)::BIGINT AS pools_priced
                FROM (
                    SELECT DISTINCT ON (protocol) tvl_usd, pools_priced
                    FROM analytic_protocol_tvl, bound
                    WHERE bucket >= bound.oldest
                    ORDER BY protocol, bucket DESC
                ) latest
            ),
            vol AS (
                SELECT
                    SUM(volume_usd) AS volume_24h_usd,
                    SUM(fees_usd)   AS fees_24h_usd,
                    COALESCE(SUM(active_pools), 0)::BIGINT
                        AS swap_buckets_24h,
                    COALESCE(SUM(active_pools_valued), 0)::BIGINT
                        AS swap_buckets_priced_24h
                FROM analytic_protocol_hourly, bound, written
                WHERE bucket >= bound.first_hour
                  AND written.first_hour <= bound.first_hour
                  AND written.last_hour >= bound.oldest
            )
            SELECT
                tvl.total_tvl_usd  AS "total_tvl_usd?",
                tvl.pools_priced   AS "pools_priced!",
                vol.volume_24h_usd AS "volume_24h_usd?",
                vol.fees_24h_usd   AS "fees_24h_usd?",
                vol.swap_buckets_24h        AS "swap_buckets_24h!",
                vol.swap_buckets_priced_24h AS "swap_buckets_priced_24h!"
            FROM tvl, vol
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        GlobalAnalytics::try_from(row)
    }
}
