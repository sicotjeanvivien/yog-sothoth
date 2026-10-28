//! PostgreSQL implementation of [`LiquidityMoveLookup`]: the valued moves yog-analytic wrote,
//! summed by protocol and hour, and listed by protocol or by pool.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::{LiquidityFlowsRow, LiquidityMoveRow};
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        FrozenPeriod, LiquidityFlows, LiquidityMove, LiquidityMoveCursor, LiquidityMoveKind,
        LiquidityMoveList, LiquidityMoveLookup, LiquidityMoveOrder, LiquidityMoveScope, Protocol,
        ProtocolBucket, Window,
    },
};

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_decimal_to_bigdecimal, map_sqlx_error,
};

pub struct PgLiquidityMoveRepository {
    pool: PgPool,
}

impl PgLiquidityMoveRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LiquidityMoveLookup for PgLiquidityMoveRepository {
    async fn find_measured_since(
        &self,
        protocol: Protocol,
    ) -> RepositoryResult<Option<DateTime<Utc>>> {
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

    async fn find_totals(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Option<LiquidityFlows>> {
        let row = sqlx::query_as!(
            LiquidityFlowsRow,
            r#"
            SELECT $3::TEXT AS "protocol!", NULL::TIMESTAMPTZ AS bucket_from,
                   SUM(created_usd) AS "created_usd!", SUM(added_usd) AS "added_usd!",
                   SUM(removed_usd) AS "removed_usd!", SUM(moves)::BIGINT AS "moves!",
                   SUM(moves_valued)::BIGINT AS "moves_valued!"
              FROM analytic_protocol_liquidity_hourly
             WHERE protocol = $3 AND bucket >= $1 AND bucket < $2
            HAVING COUNT(*) > 0
            "#,
            period.from,
            period.as_of,
            protocol.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(LiquidityFlows::try_from).transpose()
    }

    async fn list_hourly(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>> {
        let rows = sqlx::query_as!(
            LiquidityFlowsRow,
            r#"
            SELECT protocol, bucket AS "bucket_from?", created_usd, added_usd, removed_usd,
                   moves, moves_valued
              FROM analytic_protocol_liquidity_hourly
             WHERE protocol = $4
               AND bucket >= GREATEST($1::TIMESTAMPTZ, $3::TIMESTAMPTZ) AND bucket < $2
             ORDER BY bucket
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_daily(
        &self,
        period: &FrozenPeriod,
        protocol: Protocol,
    ) -> RepositoryResult<Vec<ProtocolBucket<LiquidityFlows>>> {
        let rows = sqlx::query_as!(
            LiquidityFlowsRow,
            r#"
            SELECT protocol AS "protocol!", bucket_from AS "bucket_from?",
                   SUM(created_usd) AS "created_usd!", SUM(added_usd) AS "added_usd!",
                   SUM(removed_usd) AS "removed_usd!", SUM(moves)::BIGINT AS "moves!",
                   SUM(moves_valued)::BIGINT AS "moves_valued!"
              FROM (
                  SELECT l.*,
                         time_bucket(INTERVAL '1 day', bucket, origin => $2::TIMESTAMPTZ)
                             AS bucket_from
                    FROM analytic_protocol_liquidity_hourly l
                   WHERE protocol = $4 AND bucket >= $1 AND bucket < $2
              ) hours
             WHERE bucket_from >= $3
             GROUP BY protocol, bucket_from
             ORDER BY bucket_from
            "#,
            period.from,
            period.as_of,
            period.covered_since,
            protocol.as_str(),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(ProtocolBucket::try_from).collect()
    }

    async fn list_moves(
        &self,
        period: &FrozenPeriod,
        window: Window,
        list: LiquidityMoveList,
        order: LiquidityMoveOrder,
        cursor: Option<LiquidityMoveCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityMove>> {
        let kind = list.kind.map(LiquidityMoveKind::as_str);
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
        let rows = match (list.scope, order) {
            (LiquidityMoveScope::Protocol(protocol), LiquidityMoveOrder::Amount) => sqlx::query_as!(
                LiquidityMoveRow,
                r#"
                SELECT m.protocol, m.timestamp AS at, m.signature, m.event_index,
                       m.pool_address, ta.symbol AS "token_a_symbol?",
                       tb.symbol AS "token_b_symbol?", m.owner, m.kind,
                       m.amount_usd AS "amount_usd!",
                       ROUND(m.amount_usd, 6) AS "amount_key!"
                  FROM analytic_liquidity_moves m
                  LEFT JOIN pools p ON p.pool_address = m.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 WHERE m.protocol = $3
                   AND m.timestamp >= $1 AND m.timestamp < $2
                   AND m.amount_usd IS NOT NULL
                   AND ($9::TEXT IS NULL OR m.kind = $9)
                   -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits, the column more.
                   AND ($4::NUMERIC IS NULL
                        OR ROUND(m.amount_usd, 6) < $4
                        OR (ROUND(m.amount_usd, 6) = $4 AND (m.timestamp < $5
                            OR (m.timestamp = $5 AND (m.signature, m.event_index) > ($6, $7)))))
                 ORDER BY ROUND(m.amount_usd, 6) DESC, m.timestamp DESC, m.signature,
                          m.event_index
                 LIMIT $8
                "#,
                period.from,
                period.as_of,
                protocol.as_str(),
                after_amount,
                after_at,
                after_signature,
                after_index,
                limit + 1,
                kind,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            (LiquidityMoveScope::Protocol(protocol), LiquidityMoveOrder::At) => sqlx::query_as!(
                LiquidityMoveRow,
                r#"
                SELECT m.protocol, m.timestamp AS at, m.signature, m.event_index,
                       m.pool_address, ta.symbol AS "token_a_symbol?",
                       tb.symbol AS "token_b_symbol?", m.owner, m.kind,
                       m.amount_usd AS "amount_usd!",
                       ROUND(m.amount_usd, 6) AS "amount_key!"
                  FROM analytic_liquidity_moves m
                  LEFT JOIN pools p ON p.pool_address = m.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 WHERE m.protocol = $3
                   AND m.timestamp >= $1 AND m.timestamp < $2
                   AND m.amount_usd IS NOT NULL
                   AND ($8::TEXT IS NULL OR m.kind = $8)
                   AND ($4::TIMESTAMPTZ IS NULL
                        OR m.timestamp < $4
                        OR (m.timestamp = $4 AND (m.signature, m.event_index) > ($5, $6)))
                 ORDER BY m.timestamp DESC, m.signature, m.event_index
                 LIMIT $7
                "#,
                period.from,
                period.as_of,
                protocol.as_str(),
                after_at,
                after_signature,
                after_index,
                limit + 1,
                kind,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            (LiquidityMoveScope::Pool(pool), LiquidityMoveOrder::Amount) => sqlx::query_as!(
                LiquidityMoveRow,
                r#"
                SELECT m.protocol, m.timestamp AS at, m.signature, m.event_index,
                       m.pool_address, ta.symbol AS "token_a_symbol?",
                       tb.symbol AS "token_b_symbol?", m.owner, m.kind,
                       m.amount_usd AS "amount_usd!",
                       ROUND(m.amount_usd, 6) AS "amount_key!"
                  FROM analytic_liquidity_moves m
                  LEFT JOIN pools p ON p.pool_address = m.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 WHERE m.pool_address = $3
                   AND m.timestamp >= $1 AND m.timestamp < $2
                   AND m.amount_usd IS NOT NULL
                   AND ($9::TEXT IS NULL OR m.kind = $9)
                   -- ⚠️ Rounded as the cursor holds it: a `Decimal` keeps 28 digits, the column more.
                   AND ($4::NUMERIC IS NULL
                        OR ROUND(m.amount_usd, 6) < $4
                        OR (ROUND(m.amount_usd, 6) = $4 AND (m.timestamp < $5
                            OR (m.timestamp = $5 AND (m.signature, m.event_index) > ($6, $7)))))
                 ORDER BY ROUND(m.amount_usd, 6) DESC, m.timestamp DESC, m.signature,
                          m.event_index
                 LIMIT $8
                "#,
                period.from,
                period.as_of,
                pool.to_string(),
                after_amount,
                after_at,
                after_signature,
                after_index,
                limit + 1,
                kind,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            (LiquidityMoveScope::Pool(pool), LiquidityMoveOrder::At) => sqlx::query_as!(
                LiquidityMoveRow,
                r#"
                SELECT m.protocol, m.timestamp AS at, m.signature, m.event_index,
                       m.pool_address, ta.symbol AS "token_a_symbol?",
                       tb.symbol AS "token_b_symbol?", m.owner, m.kind,
                       m.amount_usd AS "amount_usd!",
                       ROUND(m.amount_usd, 6) AS "amount_key!"
                  FROM analytic_liquidity_moves m
                  LEFT JOIN pools p ON p.pool_address = m.pool_address
                  LEFT JOIN token_metadata ta ON ta.mint = p.token_a_mint::TEXT
                  LEFT JOIN token_metadata tb ON tb.mint = p.token_b_mint::TEXT
                 WHERE m.pool_address = $3
                   AND m.timestamp >= $1 AND m.timestamp < $2
                   AND m.amount_usd IS NOT NULL
                   AND ($8::TEXT IS NULL OR m.kind = $8)
                   AND ($4::TIMESTAMPTZ IS NULL
                        OR m.timestamp < $4
                        OR (m.timestamp = $4 AND (m.signature, m.event_index) > ($5, $6)))
                 ORDER BY m.timestamp DESC, m.signature, m.event_index
                 LIMIT $7
                "#,
                period.from,
                period.as_of,
                pool.to_string(),
                after_at,
                after_signature,
                after_index,
                limit + 1,
                kind,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
        };
        let moves = rows
            .into_iter()
            .map(LiquidityMoveRow::into_keyed)
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(moves, limit, QueryMode::Forward, had_cursor).finalize(|(m, key)| {
                Cursor::LiquidityMove(Box::new(LiquidityMoveCursor {
                    list,
                    window,
                    as_of: period.as_of,
                    order,
                    amount_usd: *key,
                    at: m.at,
                    signature: m.signature.clone(),
                    event_index: m.event_index,
                }))
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            items: page.items.into_iter().map(|(m, _)| m).collect(),
            next_cursor: page.next_cursor,
            prev_cursor: None,
            is_first: page.is_first,
            is_last: page.is_last,
        })
    }
}
