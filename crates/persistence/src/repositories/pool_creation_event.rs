//! PostgreSQL implementation of [`PoolCreationEventLookup`]: each protocol's pool creation table, read by scope.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::MeteoraDammV2InitializePoolEventRow;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2InitializePoolEvent, PoolCreationEvent,
        PoolCreationEventLookup, Protocol,
    },
};

use crate::repositories::helper::{PageBuilder, QueryMode, map_sqlx_error};

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
        let had_cursor = cursor.is_some();
        let (after_at, after_signature, after_index) = match &cursor {
            Some(c) => (
                Some(c.at),
                Some(c.signature.to_string()),
                Some(i32::from(c.event_index)),
            ),
            None => (None, None, None),
        };
        let rows = match protocol {
            None | Some(Protocol::MeteoraDammV2) => sqlx::query_as!(
                MeteoraDammV2InitializePoolEventRow,
                r#"
                SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
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
                after_at,
                after_signature,
                after_index,
                limit + 1,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
            Some(Protocol::MeteoraDlmm) => return Ok(Page::empty()),
        };
        let events = rows
            .into_iter()
            .map(|row| {
                MeteoraDammV2InitializePoolEvent::try_from(row)
                    .map(PoolCreationEvent::MeteoraDammV2)
            })
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(events, limit, QueryMode::Forward, had_cursor).finalize(|event| {
                let PoolCreationEvent::MeteoraDammV2(e) = event;
                Cursor::PoolCreationEvent(Box::new(EventCursor {
                    scope: protocol.map_or(EventScope::All, EventScope::Protocol),
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
