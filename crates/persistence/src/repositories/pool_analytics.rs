//! PostgreSQL implementation of [`PoolAnalyticsRepository`].
//!
//! [keep the existing module-level doc as-is]
mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::{PoolAnalyticsRow, PoolHistoryRow};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::collections::HashMap;
use yog_core::{
    RepositoryResult,
    domain::{PoolAnalytics, PoolAnalyticsRepository, PoolHistoryBucket},
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
            tvl_per_pool AS (
                SELECT
                    pcs.pool_address,
                    (
                        (pcs.reserve_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                        +
                        (pcs.reserve_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                    ) AS tvl_usd
                FROM pool_current_state pcs
                JOIN pools p ON p.pool_address = pcs.pool_address
                JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
                JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = p.token_a_mint::TEXT
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpa ON true
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = p.token_b_mint::TEXT
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpb ON true
                WHERE pcs.pool_address = ANY($1::TEXT[])
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
}
