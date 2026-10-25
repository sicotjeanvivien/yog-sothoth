//! PostgreSQL implementation of [`LiquidityChangeEventLookup`]: each protocol's liquidity event
//! table, read by scope.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::MeteoraDammV2LiquidityEventRow;
use sqlx::PgPool;
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
            SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
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
            SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
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
