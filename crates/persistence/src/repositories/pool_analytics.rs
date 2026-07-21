//! PostgreSQL implementation of [`PoolAnalyticsRepository`].
//!
//! [keep the existing module-level doc as-is]
mod rows;

use crate::repositories::helper::{convert_string_to_pubkey, map_sqlx_error};
use async_trait::async_trait;
use rows::{PoolAnalyticsRow, PoolHistoryRow};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::collections::HashMap;
use yog_core::{
    RepositoryResult,
    domain::{PoolAnalytics, PoolAnalyticsRepository, PoolHistoryBucket, PoolRankMetric},
};

pub struct PgPoolAnalyticsRepository {
    pool: PgPool,
}

impl PgPoolAnalyticsRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolAnalyticsRepository for PgPoolAnalyticsRepository {
    async fn batch_compute(
        &self,
        pool_addresses: &[Pubkey],
    ) -> RepositoryResult<HashMap<Pubkey, PoolAnalytics>> {
        if pool_addresses.is_empty() {
            return Ok(HashMap::new());
        }

        // sqlx needs string-typed addresses to bind a `TEXT[]` array.
        let addresses: Vec<String> = pool_addresses.iter().map(|p| p.to_string()).collect();

        // [keep the existing CTE doc-comment block]
        let rows = sqlx::query_as!(
            PoolAnalyticsRow,
            r#"
            WITH
            requested AS (
                SELECT pool_address
                FROM UNNEST($1::TEXT[]) AS pool_address
            ),
            -- Per-pool current TVL is encapsulated in the `pool_current_tvl`
            -- view (migration 020) — same reserve × most-recent-price valuation
            -- as before, no longer duplicated here (the `global_analytics`
            -- roll-up reads the same view).
            tvl_per_pool AS (
                SELECT pool_address, tvl_usd
                FROM pool_current_tvl
                WHERE pool_address = ANY($1::TEXT[])
            ),
            -- 24h volume + realized fees, rolled up from the shared
            -- per-(pool, hour) USD valuation view (migration 019) — same
            -- trade-time valuation as before, no longer duplicated here.
            volume_per_pool AS (
                SELECT
                    a.pool_address,
                    SUM(a.volume_usd)        AS volume_24h_usd,
                    SUM(a.fees_usd)          AS fees_24h_usd,
                    SUM(a.protocol_fees_usd) AS protocol_fees_24h_usd
                FROM meteora_damm_v2_pool_hourly_activity a
                WHERE a.pool_address = ANY($1::TEXT[])
                  AND a.bucket > NOW() - INTERVAL '24 hours'
                GROUP BY a.pool_address
            )
            SELECT
                r.pool_address AS "pool_address!",
                t.tvl_usd      AS "tvl_usd?",
                v.volume_24h_usd AS "volume_24h_usd?",
                v.fees_24h_usd AS "fees_24h_usd?",
                v.protocol_fees_24h_usd AS "protocol_fees_24h_usd?"
            FROM requested r
            LEFT JOIN tvl_per_pool t    ON t.pool_address = r.pool_address
            LEFT JOIN volume_per_pool v ON v.pool_address = r.pool_address
            "#,
            &addresses,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(<(Pubkey, PoolAnalytics)>::try_from)
            .collect()
    }

    async fn history(
        &self,
        pool_address: &Pubkey,
        days: i32,
    ) -> RepositoryResult<Vec<PoolHistoryBucket>> {
        let address = pool_address.to_string();

        // The per-(pool, hour) USD valuation of the four CAs lives in the
        // `meteora_damm_v2_pool_hourly_activity` VIEW (migration 019) — this
        // query just slices it to one pool and window. The macro still verifies
        // these columns against the view.
        let rows = sqlx::query_as!(
            PoolHistoryRow,
            r#"
            SELECT
                bucket                AS "bucket!",
                volume_usd,
                fees_usd,
                protocol_fees_usd,
                swap_count,
                liquidity_added_usd,
                liquidity_removed_usd,
                fees_claimed_usd,
                rewards_claimed_usd
            FROM meteora_damm_v2_pool_hourly_activity
            WHERE pool_address = $1
              AND bucket > NOW() - make_interval(days => $2)
            ORDER BY bucket
            "#,
            address,
            days,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolHistoryBucket::try_from).collect()
    }

    async fn top_pool_addresses(
        &self,
        metric: PoolRankMetric,
        limit: i64,
    ) -> RepositoryResult<Vec<Pubkey>> {
        // One static, compile-checked query per metric — no dynamic SQL. Each
        // ranks over the priced analytics and drops pools with a NULL metric
        // value (unpriceable, or no activity in the window) rather than
        // sorting them last.
        let addresses: Vec<String> = match metric {
            PoolRankMetric::Volume24h => sqlx::query_scalar!(
                r#"
                SELECT pool_address AS "pool_address!"
                FROM meteora_damm_v2_pool_hourly_activity
                WHERE bucket > NOW() - INTERVAL '24 hours'
                GROUP BY pool_address
                HAVING SUM(volume_usd) IS NOT NULL
                ORDER BY SUM(volume_usd) DESC
                LIMIT $1
                "#,
                limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            // TVL is one row per pool in the `pool_current_tvl` VIEW (no
            // window/aggregate), so no GROUP BY — just filter out the
            // unpriceable pools and order by depth.
            PoolRankMetric::Tvl => sqlx::query_scalar!(
                r#"
                SELECT pool_address AS "pool_address!"
                FROM pool_current_tvl
                WHERE tvl_usd IS NOT NULL
                ORDER BY tvl_usd DESC
                LIMIT $1
                "#,
                limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
        };

        addresses
            .into_iter()
            .map(|a| convert_string_to_pubkey(a, "pool_address"))
            .collect()
    }
}
