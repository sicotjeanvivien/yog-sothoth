//! PostgreSQL implementation of [`GlobalAnalyticsRepository`].
//!
//! One query, two independent roll-ups cross-joined into a single row, each
//! reading a versioned VIEW so the heavy valuation SQL lives in migrations,
//! not here:
//!   - `tvl`: summed current TVL over every priceable pool (and the count of
//!     pools that contributed — the coverage numerator), from the
//!     `pool_current_tvl` view (migration 020). `tvl_usd` is NULL for an
//!     unpriceable pool, so the SUM skips it and the `FILTER` counts only the
//!     priced ones — partial coverage surfaces what is priceable.
//!   - `vol`: summed 24h volume and realized fees from the per-(pool, hour)
//!     USD valuation view (migration 019), windowed to the last 24h.

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
            WITH tvl AS (
                SELECT
                    SUM(tvl_usd)                                AS total_tvl_usd,
                    COUNT(*) FILTER (WHERE tvl_usd IS NOT NULL) AS pools_priced
                FROM pool_current_tvl
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
