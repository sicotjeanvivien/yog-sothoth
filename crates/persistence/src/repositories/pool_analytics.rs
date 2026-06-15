//! PostgreSQL implementation of [`PoolAnalyticsRepository`].
//!
//! [keep the existing module-level doc as-is]
mod rows;

use crate::repositories::helper::map_sqlx_error;
use async_trait::async_trait;
use rows::PoolAnalyticsRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::collections::HashMap;
use yog_core::{
    RepositoryResult,
    domain::{PoolAnalytics, PoolAnalyticsRepository},
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
            -- Volume reads the hourly continuous aggregate (migration 010)
            -- instead of scanning raw swap_events. The CA stores raw token
            -- amounts split by direction (volume_in_a from a_to_b swaps,
            -- volume_in_b from b_to_a); USD valuation stays here, per bucket,
            -- at the price as-of that bucket — preserving trade-time pricing.
            volume_per_pool AS (
                SELECT
                    h.pool_address,
                    SUM(
                        (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                        +
                        (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                    ) AS volume_24h_usd
                FROM meteora_damm_v2_swap_events_hourly h
                JOIN token_metadata tma ON tma.mint = h.token_a_mint
                JOIN token_metadata tmb ON tmb.mint = h.token_b_mint
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = h.token_a_mint
                      AND fetched_at <= h.bucket
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpa ON true
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = h.token_b_mint
                      AND fetched_at <= h.bucket
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpb ON true
                WHERE h.pool_address = ANY($1::TEXT[])
                  AND h.bucket > NOW() - INTERVAL '24 hours'
                GROUP BY h.pool_address
            )
            SELECT
                r.pool_address AS "pool_address!",
                t.tvl_usd      AS "tvl_usd?",
                v.volume_24h_usd AS "volume_24h_usd?"
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
}
