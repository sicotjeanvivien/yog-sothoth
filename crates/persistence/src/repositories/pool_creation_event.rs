//! PostgreSQL implementation of [`PoolCreationEventLookup`]: each protocol's pool creation
//! table, read by protocol.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rows::MeteoraDammV2InitializePoolEventRow;
use sqlx::PgPool;
use yog_core::domain::{LiveEventFeed, LiveRecord, PoolCreationTable};
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2InitializePoolEvent, PoolCreationEvent,
        PoolCreationEventLookup, Protocol,
    },
};

use crate::repositories::helper::{EventKey, finish_event_page, map_sqlx_error};

pub struct PgPoolCreationEventRepository {
    pool: PgPool,
}

impl PgPoolCreationEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolCreationEventLookup for PgPoolCreationEventRepository {
    async fn list_events(
        &self,
        protocol: Option<Protocol>,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<PoolCreationEvent>> {
        let key = EventKey::of(cursor.as_ref());
        let rows = match protocol {
            // Every protocol's creations: DAMM v2's table is the only one yet.
            None => self.list_meteora_damm_v2(from, &key, limit + 1).await?,
            Some(Protocol::MeteoraDammV2) => {
                self.list_meteora_damm_v2(from, &key, limit + 1).await?
            }
            Some(Protocol::MeteoraDlmm) => return Ok(Page::empty()),
        };
        let events = rows
            .into_iter()
            .map(|row| {
                MeteoraDammV2InitializePoolEvent::try_from(row)
                    .map(PoolCreationEvent::MeteoraDammV2)
            })
            .collect::<RepositoryResult<Vec<_>>>()?;
        let scope = protocol.map_or(EventScope::All, EventScope::Protocol);
        Ok(finish_event_page(
            events,
            limit,
            cursor.is_some(),
            |event| {
                let PoolCreationEvent::MeteoraDammV2(e) = event;
                Cursor::PoolCreationEvent(Box::new(EventCursor {
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
impl LiveEventFeed<PoolCreationTable, PoolCreationEvent> for PgPoolCreationEventRepository {
    async fn live_tip(
        &self,
        table: PoolCreationTable,
        lookback: Duration,
    ) -> RepositoryResult<Option<i64>> {
        match table {
            PoolCreationTable::MeteoraDammV2 => sqlx::query_scalar!(
                r#"
                SELECT max(id) AS "id"
                  FROM meteora_damm_v2_initialize_pool_events
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
        table: PoolCreationTable,
        after_id: i64,
        lookback: Duration,
        limit: i64,
    ) -> RepositoryResult<Vec<LiveRecord<RepositoryResult<PoolCreationEvent>>>> {
        let rows = match table {
            PoolCreationTable::MeteoraDammV2 => sqlx::query_as!(
                MeteoraDammV2InitializePoolEventRow,
                r#"
                SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                       token_a_mint, token_b_mint, creator, payer, alpha_vault, sqrt_min_price,
                       sqrt_max_price, sqrt_price, liquidity, activation_type, activation_point,
                       collect_fee_mode, pool_type, token_a_flag, token_b_flag, token_a_amount,
                       token_b_amount, total_amount_a, total_amount_b, pool_fees_raw
                  FROM meteora_damm_v2_initialize_pool_events
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
                event: MeteoraDammV2InitializePoolEvent::try_from(row)
                    .map(PoolCreationEvent::MeteoraDammV2),
            })
            .collect())
    }
}

impl PgPoolCreationEventRepository {
    /// DAMM v2's creations after the key.
    async fn list_meteora_damm_v2(
        &self,
        from: DateTime<Utc>,
        key: &EventKey,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2InitializePoolEventRow>> {
        sqlx::query_as!(
            MeteoraDammV2InitializePoolEventRow,
            r#"
            SELECT id, pool_address, signature, timestamp, slot, transaction_index, event_index,
                   token_a_mint, token_b_mint, creator, payer, alpha_vault, sqrt_min_price,
                   sqrt_max_price, sqrt_price, liquidity, activation_type, activation_point,
                   collect_fee_mode, pool_type, token_a_flag, token_b_flag, token_a_amount,
                   token_b_amount, total_amount_a, total_amount_b, pool_fees_raw
              FROM meteora_damm_v2_initialize_pool_events
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
}
