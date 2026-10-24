//! PostgreSQL implementation of the measure reads: yog-analytic's tables, read over the span
//! the caller built, for every protocol, one protocol, or one pool.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{
    LiquidityMoveSumsRow, PoolCreationRow, PoolDiscoveriesRow, PoolSwapBarRow, ProtocolSwapBarRow,
    ProtocolSwapTotalsRow, SwapSumsRow,
};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        ByProtocol, LiquidityMove, LiquidityMoveCursor, LiquidityMoveMeasures, LiquidityMoveOrder,
        LiquidityMoveSums, PoolCreationCount, PoolCreationMeasures, PoolDiscoveries,
        PoolDiscoveryMeasures, PoolSwapBar, Protocol, ProtocolBar, ProtocolSwapTotals, ProtocolTvl,
        Span, SwapMeasures, SwapSums, TvlReadings, Window,
    },
};

use super::protocol_activity::rows::{LiquidityMoveRow, ProtocolTvlRow};
use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, map_sqlx_error,
};

pub struct PgMeasureRepository {
    pool: PgPool,
}

impl PgMeasureRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn protocol_filter(protocol: Option<Protocol>) -> Option<&'static str> {
    protocol.map(|p| p.as_str())
}

#[async_trait]
impl SwapMeasures for PgMeasureRepository {
    async fn protocol_totals(
        &self,
        span: &Span,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolSwapTotals>> {
        let rows = sqlx::query_as!(
            ProtocolSwapTotalsRow,
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
            span.from,
            span.as_of,
            window.as_str(),
            span.last_hour(),
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolSwapTotals::try_from).collect()
    }

    async fn protocol_hourly(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<SwapSums>>> {
        let rows = sqlx::query_as!(
            ProtocolSwapBarRow,
            r#"
            SELECT protocol, bucket AS bar_from, swap_count AS swaps,
                   volume_usd, fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd,
                   active_pools AS pool_hours, active_pools_valued AS pool_hours_valued
              FROM analytic_protocol_hourly
             WHERE bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
               AND ($4::TEXT IS NULL OR protocol = $4)
             ORDER BY bucket, protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBar::try_from).collect()
    }

    async fn protocol_daily(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<SwapSums>>> {
        // ⚠️ Days aligned on `as_of`, not on midnight: the bars then add up to the span.
        let rows = sqlx::query_as!(
            ProtocolSwapBarRow,
            r#"
            SELECT protocol AS "protocol!", bar_from AS "bar_from!",
                   SUM(swap_count)::BIGINT AS "swaps!",
                   SUM(volume_usd) AS volume_usd, SUM(fees_usd) AS fees_usd,
                   SUM(lp_fees_usd) AS lp_fees_usd, SUM(protocol_fees_usd) AS protocol_fees_usd,
                   SUM(referral_fees_usd) AS referral_fees_usd,
                   SUM(active_pools)::BIGINT AS "pool_hours!",
                   SUM(active_pools_valued)::BIGINT AS "pool_hours_valued!"
              FROM (
                  SELECT h.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bar_from
                    FROM analytic_protocol_hourly h
                   WHERE bucket >= $1 AND bucket < $2
                     AND ($4::TEXT IS NULL OR protocol = $4)
              ) hours
             WHERE bar_from >= $3
             GROUP BY protocol, bar_from
             ORDER BY bar_from, protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBar::try_from).collect()
    }

    async fn pool_totals(&self, span: &Span, pool: Pubkey) -> RepositoryResult<Option<SwapSums>> {
        let row = sqlx::query_as!(
            SwapSumsRow,
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
            span.from,
            span.as_of,
            pool.to_string(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(SwapSums::try_from).transpose()
    }

    async fn pool_hourly(&self, span: &Span, pool: Pubkey) -> RepositoryResult<Vec<PoolSwapBar>> {
        let rows = sqlx::query_as!(
            PoolSwapBarRow,
            r#"
            SELECT bucket AS bar_from, swap_count AS swaps,
                   volume_in_a_usd + volume_in_b_usd AS volume_usd,
                   fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd,
                   1::BIGINT AS "pool_hours!",
                   (CASE WHEN valued THEN 1 ELSE 0 END)::BIGINT AS "pool_hours_valued!"
              FROM analytic_pool_hourly
             WHERE pool_address = $4
               AND bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
             ORDER BY bucket
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            pool.to_string(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBar::try_from).collect()
    }

    async fn pool_daily(&self, span: &Span, pool: Pubkey) -> RepositoryResult<Vec<PoolSwapBar>> {
        let rows = sqlx::query_as!(
            PoolSwapBarRow,
            r#"
            SELECT bar_from AS "bar_from!",
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
                             AS bar_from
                    FROM analytic_pool_hourly h
                   WHERE pool_address = $4 AND bucket >= $1 AND bucket < $2
              ) hours
             WHERE bar_from >= $3
             GROUP BY bar_from
             ORDER BY bar_from
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            pool.to_string(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolSwapBar::try_from).collect()
    }
}

#[async_trait]
impl PoolCreationMeasures for PgMeasureRepository {
    async fn totals(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ByProtocol<PoolCreationCount>>> {
        let rows = sqlx::query_as!(
            PoolCreationRow,
            r#"
            SELECT protocol AS "protocol!", NULL::TIMESTAMPTZ AS bar_from,
                   SUM(births)::BIGINT AS "pools!"
              FROM analytic_protocol_births_hourly
             WHERE bucket >= $1 AND bucket < $2
               AND ($3::TEXT IS NULL OR protocol = $3)
             GROUP BY protocol
             ORDER BY protocol
            "#,
            span.from,
            span.as_of,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ByProtocol::try_from).collect()
    }

    async fn hourly(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<PoolCreationCount>>> {
        let rows = sqlx::query_as!(
            PoolCreationRow,
            r#"
            SELECT protocol, bucket AS "bar_from?", births::BIGINT AS "pools!"
              FROM analytic_protocol_births_hourly
             WHERE bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
               AND ($4::TEXT IS NULL OR protocol = $4)
             ORDER BY bucket, protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBar::try_from).collect()
    }

    async fn daily(
        &self,
        span: &Span,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolBar<PoolCreationCount>>> {
        let rows = sqlx::query_as!(
            PoolCreationRow,
            r#"
            SELECT protocol AS "protocol!", bar_from AS "bar_from?",
                   SUM(births)::BIGINT AS "pools!"
              FROM (
                  SELECT b.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bar_from
                    FROM analytic_protocol_births_hourly b
                   WHERE bucket >= $1 AND bucket < $2
                     AND ($4::TEXT IS NULL OR protocol = $4)
              ) hours
             WHERE bar_from >= $3
             GROUP BY protocol, bar_from
             ORDER BY bar_from, protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBar::try_from).collect()
    }
}

#[async_trait]
impl LiquidityMoveMeasures for PgMeasureRepository {
    async fn measured_since(&self, protocol: Protocol) -> RepositoryResult<Option<DateTime<Utc>>> {
        let row = sqlx::query!(
            r#"
            SELECT MIN(bucket) AS "since?: DateTime<Utc>"
              FROM analytic_protocol_liquidity_hourly
             WHERE protocol = $1
            "#,
            protocol.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row.since)
    }

    async fn totals(
        &self,
        span: &Span,
        protocol: Protocol,
    ) -> RepositoryResult<Option<LiquidityMoveSums>> {
        let row = sqlx::query_as!(
            LiquidityMoveSumsRow,
            r#"
            SELECT $3::TEXT AS "protocol!", NULL::TIMESTAMPTZ AS bar_from,
                   SUM(created_usd) AS "created_usd!", SUM(added_usd) AS "added_usd!",
                   SUM(removed_usd) AS "removed_usd!", SUM(moves)::BIGINT AS "moves!",
                   SUM(moves_valued)::BIGINT AS "moves_valued!"
              FROM analytic_protocol_liquidity_hourly
             WHERE protocol = $3 AND bucket >= $1 AND bucket < $2
            HAVING COUNT(*) > 0
            "#,
            span.from,
            span.as_of,
            protocol.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(LiquidityMoveSums::try_from).transpose()
    }

    async fn hourly(
        &self,
        span: &Span,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBar<LiquidityMoveSums>>> {
        let rows = sqlx::query_as!(
            LiquidityMoveSumsRow,
            r#"
            SELECT protocol, bucket AS "bar_from?", created_usd, added_usd, removed_usd,
                   moves, moves_valued
              FROM analytic_protocol_liquidity_hourly
             WHERE protocol = $4
               AND bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
             ORDER BY bucket
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBar::try_from).collect()
    }

    async fn daily(
        &self,
        span: &Span,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBar<LiquidityMoveSums>>> {
        let rows = sqlx::query_as!(
            LiquidityMoveSumsRow,
            r#"
            SELECT protocol AS "protocol!", bar_from AS "bar_from?",
                   SUM(created_usd) AS "created_usd!", SUM(added_usd) AS "added_usd!",
                   SUM(removed_usd) AS "removed_usd!", SUM(moves)::BIGINT AS "moves!",
                   SUM(moves_valued)::BIGINT AS "moves_valued!"
              FROM (
                  SELECT l.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bar_from
                    FROM analytic_protocol_liquidity_hourly l
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
              ) hours
             WHERE bar_from >= $3
             GROUP BY protocol, bar_from
             ORDER BY bar_from
            "#,
            span.from,
            span.as_of,
            span.covered_since,
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBar::try_from).collect()
    }

    async fn pool_moves(
        &self,
        span: &Span,
        pool: Pubkey,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityMove>> {
        let had_cursor = cursor.is_some();
        let (after_amount, after_at, after_signature, after_index) = match cursor {
            Some(c) => (
                Some(convert_decimal_to_bigdecimal(
                    c.amount_usd,
                    "cursor amount_usd",
                )?),
                Some(c.at),
                Some(c.signature),
                Some(c.event_index),
            ),
            None => (None, None, None, None),
        };
        let rows = match order {
            LiquidityMoveOrder::Amount => sqlx::query_as!(
                LiquidityMoveRow,
                r#"
                SELECT m.protocol, m.timestamp AS at, m.signature, m.event_index,
                       m.pool_address, ta.symbol AS "token_a_symbol?",
                       tb.symbol AS "token_b_symbol?", m.owner, m.kind,
                       m.amount_usd AS "amount_usd!"
                  FROM analytic_liquidity_moves m
                  LEFT JOIN pools p ON p.pool_address = m.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 WHERE m.pool_address = $3
                   AND m.timestamp >= $1 AND m.timestamp < $2
                   AND m.amount_usd IS NOT NULL
                   AND ($4::NUMERIC IS NULL
                        OR m.amount_usd < $4
                        OR (m.amount_usd = $4 AND (m.timestamp < $5
                            OR (m.timestamp = $5 AND (m.signature, m.event_index) > ($6, $7)))))
                 ORDER BY m.amount_usd DESC, m.timestamp DESC, m.signature, m.event_index
                 LIMIT $8
                "#,
                span.from,
                span.as_of,
                pool.to_string(),
                after_amount,
                after_at,
                after_signature,
                after_index,
                limit + 1,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            LiquidityMoveOrder::At => sqlx::query_as!(
                LiquidityMoveRow,
                r#"
                SELECT m.protocol, m.timestamp AS at, m.signature, m.event_index,
                       m.pool_address, ta.symbol AS "token_a_symbol?",
                       tb.symbol AS "token_b_symbol?", m.owner, m.kind,
                       m.amount_usd AS "amount_usd!"
                  FROM analytic_liquidity_moves m
                  LEFT JOIN pools p ON p.pool_address = m.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 WHERE m.pool_address = $3
                   AND m.timestamp >= $1 AND m.timestamp < $2
                   AND m.amount_usd IS NOT NULL
                   AND ($4::TIMESTAMPTZ IS NULL
                        OR m.timestamp < $4
                        OR (m.timestamp = $4 AND (m.signature, m.event_index) > ($5, $6)))
                 ORDER BY m.timestamp DESC, m.signature, m.event_index
                 LIMIT $7
                "#,
                span.from,
                span.as_of,
                pool.to_string(),
                after_at,
                after_signature,
                after_index,
                limit + 1,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
        };
        let moves = rows
            .into_iter()
            .map(LiquidityMove::try_from)
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page = PageBuilder::new(moves, limit, QueryMode::Forward, had_cursor).finalize(|m| {
            Cursor::LiquidityMove(Box::new(LiquidityMoveCursor {
                order,
                amount_usd: m.amount_usd,
                at: m.at,
                signature: m.signature.clone(),
                event_index: m.event_index,
            }))
        });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            prev_cursor: None,
            ..page
        })
    }
}

#[async_trait]
impl TvlReadings for PgMeasureRepository {
    async fn latest(
        &self,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT DISTINCT ON (protocol)
                   protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced,
                   pools_unvalued
              FROM analytic_protocol_tvl
             WHERE bucket <= $1 AND ($2::TEXT IS NULL OR protocol = $2)
             ORDER BY protocol, bucket DESC
            "#,
            last,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }

    async fn hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced,
                   pools_unvalued
              FROM analytic_protocol_tvl
             WHERE bucket BETWEEN $1 AND $2 AND ($3::TEXT IS NULL OR protocol = $3)
             ORDER BY bucket, protocol
            "#,
            first,
            last,
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }
}

#[async_trait]
impl PoolDiscoveryMeasures for PgMeasureRepository {
    async fn totals(
        &self,
        last: DateTime<Utc>,
        window: Window,
        protocol: Option<Protocol>,
    ) -> RepositoryResult<Vec<PoolDiscoveries>> {
        let rows = sqlx::query_as!(
            PoolDiscoveriesRow,
            r#"
            SELECT DISTINCT ON (protocol)
                   protocol, bucket + INTERVAL '1 hour' AS "at!", pools, tvl_usd
              FROM analytic_protocol_tvl_discovered
             WHERE period = $2 AND bucket <= $1 AND ($3::TEXT IS NULL OR protocol = $3)
             ORDER BY protocol, bucket DESC
            "#,
            last,
            window.as_str(),
            protocol_filter(protocol),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(PoolDiscoveries::try_from).collect()
    }
}
