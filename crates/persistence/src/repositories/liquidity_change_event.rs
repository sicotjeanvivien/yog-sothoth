//! PostgreSQL implementation of [`LiquidityChangeEventLookup`]: each protocol's liquidity event table, read by scope.

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

use crate::repositories::helper::{PageBuilder, QueryMode, map_sqlx_error};

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
        let had_cursor = cursor.is_some();
        let (after_at, after_signature, after_index) = match &cursor {
            Some(c) => (
                Some(c.at),
                Some(c.signature.to_string()),
                Some(i32::from(c.event_index)),
            ),
            None => (None, None, None),
        };
        let rows = match scope {
            EventScope::All | EventScope::Protocol(Protocol::MeteoraDammV2) => sqlx::query_as!(
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
                after_at,
                after_signature,
                after_index,
                limit + 1,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            EventScope::Pool(pool) => sqlx::query_as!(
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
                pool.to_string(),
                from,
                after_at,
                after_signature,
                after_index,
                limit + 1,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            EventScope::Protocol(Protocol::MeteoraDlmm) => return Ok(Page::empty()),
        };
        let events = rows
            .into_iter()
            .map(|row| {
                MeteoraDammV2LiquidityEvent::try_from(row).map(LiquidityChangeEvent::MeteoraDammV2)
            })
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(events, limit, QueryMode::Forward, had_cursor).finalize(|event| {
                let LiquidityChangeEvent::MeteoraDammV2(e) = event;
                Cursor::LiquidityChangeEvent(Box::new(EventCursor {
                    scope,
                    from,
                    at: e.timestamp,
                    signature: e.signature,
                    event_index: e.event_index,
                }))
            });
        // ⚠️ Forward only: a previous cursor would be read forward from the first item.
        Ok(Page {
            prev_cursor: None,
            ..page
        })
    }
}
