//! PostgreSQL implementation of [`ProtocolActivityLookup`]: yog-analytic's
//! tables by protocol, read over the span the caller built. Summing the hours
//! of a span is exact for every flow; the distinct pools are read as written.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{
    LatestProtocolTvlRow, LiquidityMoveRow, ProtocolActivityBarRow, ProtocolActivityRow,
    ProtocolTvlRow, fold_latest,
};
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        FrozenHours, LatestProtocolTvl, LiquidityMove, Protocol, ProtocolActivity,
        ProtocolActivityBar, ProtocolActivityLookup, ProtocolTvl, Span, Window,
    },
};

use crate::repositories::helper::map_sqlx_error;

pub struct PgProtocolActivityRepository {
    pool: PgPool,
}

impl PgProtocolActivityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProtocolActivityLookup for PgProtocolActivityRepository {
    async fn frozen_hours(&self) -> RepositoryResult<Option<FrozenHours>> {
        let row = sqlx::query!(
            r#"
            SELECT MIN(bucket) AS "first?: DateTime<Utc>",
                   MAX(bucket) FILTER (WHERE frozen_at IS NOT NULL)
                       AS "last_frozen?: DateTime<Utc>"
              FROM analytic_hours
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(row
            .first
            .zip(row.last_frozen)
            .map(|(first, last_frozen)| FrozenHours { first, last_frozen }))
    }

    async fn activity(
        &self,
        span: &Span,
        window: Window,
    ) -> RepositoryResult<Vec<ProtocolActivity>> {
        let rows = sqlx::query_as!(
            ProtocolActivityRow,
            r#"
            WITH facts AS (
                SELECT protocol,
                       SUM(volume_usd) AS volume_usd, SUM(fees_usd) AS fees_usd,
                       SUM(lp_fees_usd) AS lp_fees_usd,
                       SUM(protocol_fees_usd) AS protocol_fees_usd,
                       SUM(referral_fees_usd) AS referral_fees_usd,
                       SUM(swap_count)::BIGINT AS swap_count,
                       SUM(active_pools)::BIGINT AS pool_hours,
                       SUM(active_pools_valued)::BIGINT AS pool_hours_valued
                  FROM analytic_protocol_hourly
                 WHERE bucket >= $1 AND bucket < $2
                 GROUP BY protocol
            ),
            births AS (
                SELECT protocol, SUM(births)::BIGINT AS births
                  FROM analytic_protocol_births_hourly
                 WHERE bucket >= $1 AND bucket < $2
                 GROUP BY protocol
            ),
            liquidity AS (
                SELECT protocol, SUM(added_usd) AS added_usd, SUM(removed_usd) AS removed_usd,
                       SUM(moves)::BIGINT AS moves, SUM(moves_valued)::BIGINT AS moves_valued
                  FROM analytic_protocol_liquidity_hourly
                 WHERE bucket >= $1 AND bucket < $2
                 GROUP BY protocol
            ),
            -- ⚠️ A full join: an hour of births without a swap still counts.
            joined AS (
                SELECT COALESCE(f.protocol, b.protocol) AS protocol,
                       f.volume_usd, f.fees_usd, f.lp_fees_usd,
                       f.protocol_fees_usd, f.referral_fees_usd,
                       f.swap_count, f.pool_hours, f.pool_hours_valued, b.births
                  FROM facts f
                  FULL JOIN births b ON b.protocol = f.protocol
            )
            SELECT j.protocol AS "protocol!",
                   j.volume_usd, j.fees_usd, j.lp_fees_usd,
                   j.protocol_fees_usd, j.referral_fees_usd,
                   COALESCE(j.swap_count, 0) AS "swap_count!",
                   p.active_pools AS "active_pools?",
                   COALESCE(j.births, 0) AS "births!",
                   COALESCE(j.pool_hours, 0) AS "pool_hours!",
                   COALESCE(j.pool_hours_valued, 0) AS "pool_hours_valued!",
                   -- ⚠️ Only a span its liquidity covers whole: before, a sum would read low.
                   CASE WHEN s.since <= $1 THEN l.added_usd END AS liquidity_added_usd,
                   CASE WHEN s.since <= $1 THEN l.removed_usd END AS liquidity_removed_usd,
                   CASE WHEN s.since <= $1 THEN l.moves END AS liquidity_moves,
                   CASE WHEN s.since <= $1 THEN l.moves_valued END AS liquidity_moves_valued
              FROM joined j
              LEFT JOIN analytic_protocol_periods p
                     ON p.protocol = j.protocol
                    AND p.period = $3
                    AND p.bucket = $4
              LEFT JOIN liquidity l ON l.protocol = j.protocol
              LEFT JOIN LATERAL (
                  SELECT MIN(bucket) AS since FROM analytic_protocol_liquidity_hourly
                   WHERE protocol = j.protocol
              ) s ON true
             ORDER BY j.protocol
            "#,
            span.from,
            span.as_of,
            window.as_str(),
            span.last_hour(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolActivity::try_from).collect()
    }

    async fn activity_hourly(&self, span: &Span) -> RepositoryResult<Vec<ProtocolActivityBar>> {
        let rows = sqlx::query_as!(
            ProtocolActivityBarRow,
            r#"
            SELECT h.protocol, h.bucket AS bar_from, h.volume_usd, h.fees_usd, h.swap_count,
                   h.active_pools AS pool_hours, h.active_pools_valued AS pool_hours_valued,
                   l.added_usd AS "liquidity_added_usd?", l.removed_usd AS "liquidity_removed_usd?",
                   l.moves AS "liquidity_moves?", l.moves_valued AS "liquidity_moves_valued?"
              FROM analytic_protocol_hourly h
              LEFT JOIN analytic_protocol_liquidity_hourly l
                     ON l.protocol = h.protocol AND l.bucket = h.bucket
             WHERE h.bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND h.bucket < $2
             ORDER BY h.bucket, h.protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolActivityBar::try_from)
            .collect()
    }

    async fn activity_daily(&self, span: &Span) -> RepositoryResult<Vec<ProtocolActivityBar>> {
        // ⚠️ Days aligned on `as_of`, not on midnight: the bars then add up to the span.
        let rows = sqlx::query_as!(
            ProtocolActivityBarRow,
            r#"
            SELECT hours.protocol, hours.bar_from AS "bar_from!",
                   SUM(hours.volume_usd) AS volume_usd, SUM(hours.fees_usd) AS fees_usd,
                   SUM(hours.swap_count)::BIGINT AS "swap_count!",
                   SUM(hours.active_pools)::BIGINT AS "pool_hours!",
                   SUM(hours.active_pools_valued)::BIGINT AS "pool_hours_valued!",
                   -- ⚠️ Only a bar its liquidity covers whole: before, a sum would read low.
                   CASE WHEN hours.bar_from >= MIN(s.since) THEN SUM(l.added_usd) END
                       AS liquidity_added_usd,
                   CASE WHEN hours.bar_from >= MIN(s.since) THEN SUM(l.removed_usd) END
                       AS liquidity_removed_usd,
                   CASE WHEN hours.bar_from >= MIN(s.since) THEN SUM(l.moves)::BIGINT END
                       AS liquidity_moves,
                   CASE WHEN hours.bar_from >= MIN(s.since) THEN SUM(l.moves_valued)::BIGINT END
                       AS liquidity_moves_valued
              FROM (
                  SELECT h.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bar_from
                    FROM analytic_protocol_hourly h
                   WHERE bucket >= $1 AND bucket < $2
              ) hours
              LEFT JOIN analytic_protocol_liquidity_hourly l
                     ON l.protocol = hours.protocol AND l.bucket = hours.bucket
              LEFT JOIN LATERAL (
                  SELECT MIN(bucket) AS since FROM analytic_protocol_liquidity_hourly
                   WHERE protocol = hours.protocol
              ) s ON true
             WHERE hours.bar_from >= $3
             GROUP BY hours.protocol, hours.bar_from
             ORDER BY hours.bar_from, hours.protocol
            "#,
            span.from,
            span.as_of,
            span.covered_since,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(ProtocolActivityBar::try_from)
            .collect()
    }

    async fn tvl_latest(&self, last: DateTime<Utc>) -> RepositoryResult<Vec<LatestProtocolTvl>> {
        // ⚠️ The parts joined on the TVL row's own hour: a part of another hour splits another TVL.
        let rows = sqlx::query_as!(
            LatestProtocolTvlRow,
            r#"
            WITH latest AS (
                SELECT DISTINCT ON (protocol)
                       protocol, bucket, tvl_usd, pools_priced, pools_unvalued
                  FROM analytic_protocol_tvl
                 WHERE bucket <= $1
                 ORDER BY protocol, bucket DESC
            )
            SELECT l.protocol AS "protocol!", l.bucket + INTERVAL '1 hour' AS "at!",
                   l.tvl_usd, l.pools_priced AS "pools_priced!",
                   l.pools_unvalued AS "pools_unvalued!",
                   d.period AS "discovered_period?", d.tvl_usd AS "discovered_usd?",
                   d.pools AS "discovered_pools?"
              FROM latest l
              LEFT JOIN analytic_protocol_tvl_discovered d
                     ON d.protocol = l.protocol AND d.bucket = l.bucket
             ORDER BY l.protocol
            "#,
            last,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        fold_latest(rows)
    }

    async fn tvl_hourly(
        &self,
        first: DateTime<Utc>,
        last: DateTime<Utc>,
    ) -> RepositoryResult<Vec<ProtocolTvl>> {
        let rows = sqlx::query_as!(
            ProtocolTvlRow,
            r#"
            SELECT protocol, bucket + INTERVAL '1 hour' AS "at!", tvl_usd, pools_priced,
                   pools_unvalued
              FROM analytic_protocol_tvl
             WHERE bucket BETWEEN $1 AND $2
             ORDER BY bucket, protocol
            "#,
            first,
            last,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolTvl::try_from).collect()
    }

    async fn liquidity_moves(
        &self,
        span: &Span,
        protocol: Protocol,
        limit: i64,
    ) -> RepositoryResult<Vec<LiquidityMove>> {
        let rows = sqlx::query_as!(
            LiquidityMoveRow,
            r#"
            SELECT m.protocol, m.timestamp AS at, m.signature, m.pool_address,
                   ta.symbol AS "token_a_symbol?", tb.symbol AS "token_b_symbol?",
                   m.owner, m.kind, m.amount_usd AS "amount_usd!"
              FROM analytic_liquidity_moves m
              LEFT JOIN pools p ON p.pool_address = m.pool_address
              LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
              LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
             WHERE m.protocol = $3
               AND m.timestamp >= $1 AND m.timestamp < $2
               AND m.amount_usd IS NOT NULL
               -- ⚠️ Only a span its liquidity covers whole, as the totals: else a ranking of part of it.
               AND (SELECT MIN(bucket) FROM analytic_protocol_liquidity_hourly
                     WHERE protocol = $3) <= $1
             ORDER BY m.amount_usd DESC, m.timestamp DESC, m.signature, m.event_index
             LIMIT $4
            "#,
            span.from,
            span.as_of,
            protocol.as_str(),
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(LiquidityMove::try_from).collect()
    }
}
