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
            -- Volume reads the hourly continuous aggregate (no mints on it —
            -- they're a pool property) and joins `pools` for the mints. USD
            -- valuation stays here, per bucket, at the price as-of that bucket
            -- — preserving trade-time pricing. The CA stores raw token amounts
            -- split by direction (volume_in_a from a_to_b, volume_in_b from
            -- b_to_a). Pools whose mints aren't resolved yet drop out (INNER
            -- JOIN on token_metadata) and yield a NULL volume.
            volume_per_pool AS (
                SELECT
                    h.pool_address,
                    SUM(
                        (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                        +
                        (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                    ) AS volume_24h_usd,
                    -- Realized trading fee and the protocol's share, valued the
                    -- same way (raw token amounts split by the token that bore
                    -- the fee, priced as-of each bucket). The CA columns come
                    -- from migration 017.
                    SUM(
                        (COALESCE(h.fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                        +
                        (COALESCE(h.fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                    ) AS fees_24h_usd,
                    SUM(
                        (COALESCE(h.protocol_fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                        +
                        (COALESCE(h.protocol_fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                    ) AS protocol_fees_24h_usd
                FROM meteora_damm_v2_swap_events_hourly h
                JOIN pools p ON p.pool_address = h.pool_address
                JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
                JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = p.token_a_mint::TEXT
                      AND fetched_at <= h.bucket
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpa ON true
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = p.token_b_mint::TEXT
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

        // One hourly time-series for the pool, built from the four CAs joined
        // on the bucket. Each source is valued in USD at the token price as-of
        // its bucket (trade-time valuation, like `batch_compute`): swap/
        // liquidity/position-fee use the pool's two mints (from `pools` +
        // `token_metadata`); reward claims use the reward mint and are summed
        // across mints per bucket. The bucket spine is the UNION of every
        // source's buckets, LEFT-joined back so a bucket active in one source
        // but not another still appears (other columns NULL).
        let rows = sqlx::query_as!(
            PoolHistoryRow,
            r#"
            WITH
            pool_tokens AS (
                SELECT p.pool_address, p.token_a_mint, p.token_b_mint,
                       tma.decimals AS dec_a, tmb.decimals AS dec_b
                FROM pools p
                JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
                JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
                WHERE p.pool_address = $1
            ),
            swap_h AS (
                SELECT h.bucket,
                    (COALESCE(h.volume_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
                  + (COALESCE(h.volume_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS volume_usd,
                    (COALESCE(h.fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
                  + (COALESCE(h.fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_usd,
                    (COALESCE(h.protocol_fee_in_a, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
                  + (COALESCE(h.protocol_fee_in_b, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS protocol_fees_usd,
                    h.swap_count
                FROM meteora_damm_v2_swap_events_hourly h
                JOIN pool_tokens pt ON pt.pool_address = h.pool_address
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
                WHERE h.pool_address = $1 AND h.bucket > NOW() - make_interval(days => $2)
            ),
            liq_h AS (
                SELECT h.bucket,
                    (COALESCE(h.amount_a_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
                  + (COALESCE(h.amount_b_added, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS liquidity_added_usd,
                    (COALESCE(h.amount_a_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
                  + (COALESCE(h.amount_b_removed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS liquidity_removed_usd
                FROM meteora_damm_v2_liquidity_events_hourly h
                JOIN pool_tokens pt ON pt.pool_address = h.pool_address
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
                WHERE h.pool_address = $1 AND h.bucket > NOW() - make_interval(days => $2)
            ),
            pos_fee_h AS (
                SELECT h.bucket,
                    (COALESCE(h.fee_a_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_a)) * pa.price_usd
                  + (COALESCE(h.fee_b_claimed, 0)::NUMERIC / POWER(10::NUMERIC, pt.dec_b)) * pb.price_usd AS fees_claimed_usd
                FROM meteora_damm_v2_claim_position_fee_events_hourly h
                JOIN pool_tokens pt ON pt.pool_address = h.pool_address
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_a_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pa ON true
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = pt.token_b_mint::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pb ON true
                WHERE h.pool_address = $1 AND h.bucket > NOW() - make_interval(days => $2)
            ),
            reward_h AS (
                SELECT h.bucket,
                    SUM((COALESCE(h.total_reward, 0)::NUMERIC / POWER(10::NUMERIC, tmr.decimals)) * pr.price_usd) AS rewards_claimed_usd
                FROM meteora_damm_v2_claim_reward_events_hourly h
                JOIN token_metadata tmr ON tmr.mint = h.mint_reward::TEXT
                LEFT JOIN LATERAL (SELECT price_usd FROM token_prices WHERE mint = h.mint_reward::TEXT AND fetched_at <= h.bucket ORDER BY fetched_at DESC LIMIT 1) pr ON true
                WHERE h.pool_address = $1 AND h.bucket > NOW() - make_interval(days => $2)
                GROUP BY h.bucket
            ),
            buckets AS (
                SELECT bucket FROM swap_h
                UNION SELECT bucket FROM liq_h
                UNION SELECT bucket FROM pos_fee_h
                UNION SELECT bucket FROM reward_h
            )
            SELECT
                b.bucket                  AS "bucket!",
                s.volume_usd              AS "volume_usd?",
                s.fees_usd                AS "fees_usd?",
                s.protocol_fees_usd       AS "protocol_fees_usd?",
                s.swap_count              AS "swap_count?",
                l.liquidity_added_usd     AS "liquidity_added_usd?",
                l.liquidity_removed_usd   AS "liquidity_removed_usd?",
                pf.fees_claimed_usd       AS "fees_claimed_usd?",
                rw.rewards_claimed_usd    AS "rewards_claimed_usd?"
            FROM buckets b
            LEFT JOIN swap_h s     ON s.bucket = b.bucket
            LEFT JOIN liq_h l      ON l.bucket = b.bucket
            LEFT JOIN pos_fee_h pf ON pf.bucket = b.bucket
            LEFT JOIN reward_h rw  ON rw.bucket = b.bucket
            ORDER BY b.bucket
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
