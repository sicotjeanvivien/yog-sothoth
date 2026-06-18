//! PostgreSQL implementation of [`GlobalAnalyticsRepository`].
//!
//! One query, two independent roll-ups cross-joined into a single row:
//!   - `tvl`: summed current TVL over every priceable pool (and the count of
//!     pools that contributed — the coverage numerator), valued like the
//!     per-pool `pool_analytics` TVL (current reserves × most-recent price).
//!   - `vol`: summed 24h volume and realized fees from the per-(pool, hour)
//!     USD valuation view (migration 019), windowed to the last 24h.
//!
//! Pools whose mints aren't resolved drop out of `tvl` via the INNER join on
//! `token_metadata`, exactly like the per-pool path; the SUMs skip NULL-priced
//! rows, so partial coverage surfaces what is priceable rather than collapsing
//! to NULL.

use crate::repositories::helper::{convert_bigdecimal_to_decimal, map_sqlx_error};
use async_trait::async_trait;
use bigdecimal::BigDecimal;
use sqlx::PgPool;
use yog_core::{
    RepositoryError, RepositoryResult,
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

/// Row shape for the single-row aggregate query. NUMERIC sums map to
/// `BigDecimal`; the priced-pool count is a non-null `BIGINT`.
struct GlobalAnalyticsRow {
    total_tvl_usd: Option<BigDecimal>,
    pools_priced: i64,
    volume_24h_usd: Option<BigDecimal>,
    fees_24h_usd: Option<BigDecimal>,
}

impl TryFrom<GlobalAnalyticsRow> for GlobalAnalytics {
    type Error = RepositoryError;

    fn try_from(row: GlobalAnalyticsRow) -> Result<Self, Self::Error> {
        let usd = |v: Option<BigDecimal>, field| {
            v.map(|v| convert_bigdecimal_to_decimal(v, field))
                .transpose()
        };
        Ok(GlobalAnalytics {
            total_tvl_usd: usd(row.total_tvl_usd, "total_tvl_usd")?,
            pools_priced: row.pools_priced,
            volume_24h_usd: usd(row.volume_24h_usd, "volume_24h_usd")?,
            fees_24h_usd: usd(row.fees_24h_usd, "fees_24h_usd")?,
        })
    }
}

#[async_trait]
impl GlobalAnalyticsRepository for PgGlobalAnalyticsRepository {
    async fn global_analytics(&self) -> RepositoryResult<GlobalAnalytics> {
        let row = sqlx::query_as!(
            GlobalAnalyticsRow,
            r#"
            WITH tvl AS (
                SELECT
                    SUM(
                        (pcs.reserve_a::NUMERIC / POWER(10::NUMERIC, tma.decimals)) * tpa.price_usd
                      + (pcs.reserve_b::NUMERIC / POWER(10::NUMERIC, tmb.decimals)) * tpb.price_usd
                    ) AS total_tvl_usd,
                    COUNT(*) FILTER (
                        WHERE tpa.price_usd IS NOT NULL AND tpb.price_usd IS NOT NULL
                    ) AS pools_priced
                FROM pool_current_state pcs
                JOIN pools p ON p.pool_address = pcs.pool_address
                JOIN token_metadata tma ON tma.mint = p.token_a_mint::TEXT
                JOIN token_metadata tmb ON tmb.mint = p.token_b_mint::TEXT
                LEFT JOIN LATERAL (
                    SELECT price_usd FROM token_prices
                    WHERE mint = p.token_a_mint::TEXT
                    ORDER BY fetched_at DESC LIMIT 1
                ) tpa ON true
                LEFT JOIN LATERAL (
                    SELECT price_usd FROM token_prices
                    WHERE mint = p.token_b_mint::TEXT
                    ORDER BY fetched_at DESC LIMIT 1
                ) tpb ON true
            ),
            vol AS (
                SELECT
                    SUM(volume_usd) AS volume_24h_usd,
                    SUM(fees_usd)   AS fees_24h_usd
                FROM meteora_damm_v2_pool_hourly_activity
                WHERE bucket > NOW() - INTERVAL '24 hours'
            )
            SELECT
                tvl.total_tvl_usd  AS "total_tvl_usd?",
                tvl.pools_priced   AS "pools_priced!",
                vol.volume_24h_usd AS "volume_24h_usd?",
                vol.fees_24h_usd   AS "fees_24h_usd?"
            FROM tvl, vol
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        GlobalAnalytics::try_from(row)
    }
}
