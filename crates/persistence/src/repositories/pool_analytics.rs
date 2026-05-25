//! PostgreSQL implementation of [`PoolAnalyticsRepository`].
//!
//! The interesting work is in the SQL. Two metrics are computed
//! per pool in a single round-trip:
//!
//! ## TVL
//!
//! `(reserve_a / 10^decimals_a) * latest_price_a
//!  + (reserve_b / 10^decimals_b) * latest_price_b`
//!
//! using the reserves from `pool_current_state` and the most recent
//! price for each mint in `token_prices`. If either side has no
//! known price, the pool's TVL is NULL — partial TVL would be
//! actively misleading.
//!
//! ## Volume 24h
//!
//! For every swap of the pool in the last 24h, the trader-sent side
//! is multiplied by the price *as of that swap's timestamp*. The
//! `JOIN LATERAL ... LIMIT 1` pattern keeps the price lookup bounded
//! by the index `(mint, fetched_at DESC)`. Swaps whose token had no
//! known price at swap time contribute NULL, which `SUM` ignores —
//! the resulting volume is therefore "partial but never wrong", as
//! agreed upstream.
//!
//! ## Batching
//!
//! Both calculations are wrapped in CTEs filtered by `pool_address
//! = ANY($1)`, so a single execution computes analytics for the
//! whole page of pools handed in by the caller.

use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use bigdecimal::BigDecimal;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{PoolAnalytics, PoolAnalyticsRepository},
};

use crate::repository_utils::map_sqlx_error;

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

        // Two CTEs feed the final SELECT.
        //
        // - `tvl_per_pool` walks the requested pools through
        //   pool_current_state and resolves the latest price per
        //   token. The product is `NULL` whenever either side lacks
        //   a price, which carries through to the SUM.
        //
        // - `volume_per_pool` aggregates swap_events from the last
        //   24h. For each row, two LATERAL joins fetch the token A
        //   and token B latest known price *as of the swap's
        //   timestamp* — the `<=` filter plus `ORDER BY fetched_at
        //   DESC LIMIT 1` makes the lookup an index seek.
        //
        // The outer SELECT yields one row per requested pool,
        // including pools for which no analytics row exists (LEFT
        // JOIN). Rows with NULL aggregates are returned with NULL
        // values; the caller resolves them to PoolAnalytics::empty().
        let rows = sqlx::query!(
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
            volume_per_pool AS (
                SELECT
                    s.pool_address,
                    SUM(
                        CASE
                            WHEN s.trade_direction = 'a_to_b' THEN
                                (s.amount_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                            WHEN s.trade_direction = 'b_to_a' THEN
                                (s.amount_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                        END
                    ) AS volume_24h_usd
                FROM swap_events s
                JOIN token_metadata tma ON tma.mint = s.token_a_mint
                JOIN token_metadata tmb ON tmb.mint = s.token_b_mint
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = s.token_a_mint
                      AND fetched_at <= s.timestamp
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpa ON true
                LEFT JOIN LATERAL (
                    SELECT price_usd
                    FROM token_prices
                    WHERE mint = s.token_b_mint
                      AND fetched_at <= s.timestamp
                    ORDER BY fetched_at DESC
                    LIMIT 1
                ) tpb ON true
                WHERE s.pool_address = ANY($1::TEXT[])
                  AND s.timestamp > NOW() - INTERVAL '24 hours'
                GROUP BY s.pool_address
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

        let mut out = HashMap::with_capacity(rows.len());
        for row in rows {
            let address = Pubkey::from_str(&row.pool_address).map_err(|e| {
                RepositoryError::Integrity(format!(
                    "invalid pool_address from analytics query: {e}"
                ))
            })?;

            out.insert(
                address,
                PoolAnalytics {
                    tvl_usd: row
                        .tvl_usd
                        .map(|v| bigdecimal_to_decimal(v, "tvl_usd"))
                        .transpose()?,
                    volume_24h_usd: row
                        .volume_24h_usd
                        .map(|v| bigdecimal_to_decimal(v, "volume_24h_usd"))
                        .transpose()?,
                },
            );
        }

        Ok(out)
    }
}

/// Local converter — `rust_decimal::Decimal` is the domain currency
/// type, sqlx hands us a `bigdecimal::BigDecimal`. Going through a
/// string round-trip is dull but correct.
fn bigdecimal_to_decimal(value: BigDecimal, field: &str) -> RepositoryResult<Decimal> {
    Decimal::from_str(&value.to_string()).map_err(|e| {
        RepositoryError::Integrity(format!(
            "failed to convert {field} from BigDecimal to Decimal: {e}"
        ))
    })
}
