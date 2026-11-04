//! PostgreSQL implementation of [`LiquidityChangeEventLookup`]: each protocol's liquidity event
//! table, read by scope.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rows::MeteoraDammV2LiquidityEventRow;
use sqlx::PgPool;
use yog_core::domain::{LiquidityChangeTable, LiveEventFeed, LiveRecord};
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, LiquidityChangeEvent, LiquidityChangeEventLookup,
        MeteoraDammV2LiquidityEvent, Protocol,
    },
};

use crate::repositories::helper::{EventKey, finish_event_page, map_sqlx_error};

pub struct PgLiquidityChangeEventRepository {
    pool: PgPool,
}

impl PgLiquidityChangeEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LiquidityChangeEventLookup for PgLiquidityChangeEventRepository {
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityChangeEvent>> {
        let key = EventKey::of(cursor.as_ref());
        let rows = match scope {
            // Every protocol's changes: DAMM v2's table is the only one yet.
            EventScope::All => self.list_meteora_damm_v2(from, &key, limit + 1).await?,
            EventScope::Protocol(Protocol::MeteoraDammV2) => {
                self.list_meteora_damm_v2(from, &key, limit + 1).await?
            }
            EventScope::Protocol(Protocol::MeteoraDlmm) => return Ok(Page::empty()),
            EventScope::Pool(pool) => {
                self.list_meteora_damm_v2_pool(&pool.to_string(), from, &key, limit + 1)
                    .await?
            }
        };
        let events = rows
            .into_iter()
            .map(|row| {
                MeteoraDammV2LiquidityEvent::try_from(row).map(LiquidityChangeEvent::MeteoraDammV2)
            })
            .collect::<RepositoryResult<Vec<_>>>()?;
        Ok(finish_event_page(
            events,
            limit,
            cursor.is_some(),
            |event| {
                let LiquidityChangeEvent::MeteoraDammV2(e) = event;
                Cursor::LiquidityChangeEvent(Box::new(EventCursor {
                    scope,
                    from,
                    at: e.timestamp,
                    signature: e.signature,
                    event_index: e.event_index,
                }))
            },
        ))
    }
}

#[async_trait]
impl LiveEventFeed<LiquidityChangeTable, LiquidityChangeEvent>
    for PgLiquidityChangeEventRepository
{
    async fn live_tip(
        &self,
        table: LiquidityChangeTable,
        lookback: Duration,
    ) -> RepositoryResult<Option<i64>> {
        match table {
            LiquidityChangeTable::MeteoraDammV2 => sqlx::query_scalar!(
                r#"
                SELECT max(id) AS "id"
                  FROM meteora_damm_v2_liquidity_events
                 WHERE timestamp >= now() - $1::BIGINT * INTERVAL '1 second'
                "#,
                lookback.num_seconds(),
            )
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error),
        }
    }

    async fn live_after(
        &self,
        table: LiquidityChangeTable,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<LiquidityChangeEvent>>>> {
        let rows = match table {
            LiquidityChangeTable::MeteoraDammV2 => sqlx::query_as!(
                MeteoraDammV2LiquidityEventRow,
                r#"
                SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                       liquidity_event_kind, amount_a, amount_b, liquidity_delta, reserve_a_after,
                       reserve_b_after, position, owner
                  FROM meteora_damm_v2_liquidity_events
                 WHERE id > $1
                   AND timestamp >= now() - $2::BIGINT * INTERVAL '1 second'
                 ORDER BY id
                 LIMIT $3
                "#,
                after_id,
                lookback.num_seconds(),
                limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
        };
        Ok(rows
            .into_iter()
            .map(|row| LiveRecord {
                id: row.id,
                event: MeteoraDammV2LiquidityEvent::try_from(row)
                    .map(LiquidityChangeEvent::MeteoraDammV2),
            })
            .collect())
    }
}

impl PgLiquidityChangeEventRepository {
    /// DAMM v2's changes after the key.
    async fn list_meteora_damm_v2(
        &self,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2LiquidityEventRow>> {
        sqlx::query_as!(
            MeteoraDammV2LiquidityEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   liquidity_event_kind, amount_a, amount_b, liquidity_delta, reserve_a_after,
                   reserve_b_after, position, owner
              FROM meteora_damm_v2_liquidity_events
             WHERE timestamp >= $1
               AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($3, $4))
             ORDER BY timestamp DESC, signature DESC, event_index DESC
             LIMIT $5
            "#,
            from,
            key.at,
            key.signature,
            key.event_index,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    /// One DAMM v2 pool's changes after the key.
    async fn list_meteora_damm_v2_pool(
        &self,
        pool: &str,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2LiquidityEventRow>> {
        sqlx::query_as!(
            MeteoraDammV2LiquidityEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   liquidity_event_kind, amount_a, amount_b, liquidity_delta, reserve_a_after,
                   reserve_b_after, position, owner
              FROM meteora_damm_v2_liquidity_events
             WHERE pool_address = $1 AND timestamp >= $2
               AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($4, $5))
             ORDER BY timestamp DESC, signature DESC, event_index DESC
             LIMIT $6
            "#,
            pool,
            from,
            key.at,
            key.signature,
            key.event_index,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}
