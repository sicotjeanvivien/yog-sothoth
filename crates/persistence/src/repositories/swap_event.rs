//! PostgreSQL implementation of [`SwapEventLookup`]: each protocol's swap table, read by scope.

mod rows;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rows::MeteoraDammV2SwapEventRow;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryResult,
    domain::{
        EventCursor, EventScope, MeteoraDammV2SwapEvent, Protocol, SwapEvent, SwapEventLookup,
    },
};

use crate::repositories::helper::{PageBuilder, QueryMode, map_sqlx_error};

pub struct PgSwapEventRepository {
    pool: PgPool,
}

impl PgSwapEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SwapEventLookup for PgSwapEventRepository {
    async fn list_events(
        &self,
        scope: EventScope,
        from: DateTime<Utc>,
        cursor: Option<EventCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<SwapEvent>> {
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
            EventScope::All | EventScope::Protocol(Protocol::MeteoraDammV2) => {
                self.list_all(from, after_at, after_signature, after_index, limit + 1)
                    .await?
            }
            EventScope::Pool(pool) => {
                self.list_pool(
                    &pool.to_string(),
                    from,
                    after_at,
                    after_signature,
                    after_index,
                    limit + 1,
                )
                .await?
            }
            EventScope::Protocol(Protocol::MeteoraDlmm) => return Ok(Page::empty()),
        };
        let events = rows
            .into_iter()
            .map(|row| MeteoraDammV2SwapEvent::try_from(row).map(SwapEvent::MeteoraDammV2))
            .collect::<RepositoryResult<Vec<_>>>()?;
        let page =
            PageBuilder::new(events, limit, QueryMode::Forward, had_cursor).finalize(|event| {
                let SwapEvent::MeteoraDammV2(e) = event;
                Cursor::SwapEvent(Box::new(EventCursor {
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

impl PgSwapEventRepository {
    /// Every pool's swaps after the key, in two reads: the instant of the `limit`-th one by the
    /// index of `timestamp`, then only the swaps from it on, sorted on the whole key.
    /// ⚠️ In one read, the tie-break by signature sorts every swap of the window.
    async fn list_all(
        &self,
        from: DateTime<Utc>,
        after_at: Option<DateTime<Utc>>,
        after_signature: Option<String>,
        after_index: Option<i32>,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2SwapEventRow>> {
        let floor = sqlx::query_scalar!(
            r#"
            SELECT timestamp
              FROM meteora_damm_v2_swap_events
             WHERE timestamp >= $1
               AND timestamp <= COALESCE($2, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($2, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($3, $4))
             ORDER BY timestamp DESC
             OFFSET $5::BIGINT - 1
             LIMIT 1
            "#,
            from,
            after_at,
            after_signature.clone(),
            after_index,
            limit,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        sqlx::query_as!(
            MeteoraDammV2SwapEventRow,
            r#"
            SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
                   trade_direction, amount_a, amount_b, reserve_a_after, reserve_b_after,
                   next_sqrt_price, claiming_fee, protocol_fee, compounding_fee, referral_fee,
                   fee_token_is_a
              FROM meteora_damm_v2_swap_events
             WHERE timestamp >= GREATEST($1, COALESCE($2::TIMESTAMPTZ, $1))
               AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($4, $5))
             ORDER BY timestamp DESC, signature DESC, event_index DESC
             LIMIT $6
            "#,
            from,
            floor,
            after_at,
            after_signature,
            after_index,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }

    /// One pool's swaps after the key, in the same two reads as [`Self::list_all`], on the
    /// index of `(pool_address, timestamp)`.
    async fn list_pool(
        &self,
        pool: &str,
        from: DateTime<Utc>,
        after_at: Option<DateTime<Utc>>,
        after_signature: Option<String>,
        after_index: Option<i32>,
        limit: i64,
    ) -> RepositoryResult<Vec<MeteoraDammV2SwapEventRow>> {
        let floor = sqlx::query_scalar!(
            r#"
            SELECT timestamp
              FROM meteora_damm_v2_swap_events
             WHERE pool_address = $1 AND timestamp >= $2
               AND timestamp <= COALESCE($3, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($3, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($4, $5))
             ORDER BY timestamp DESC
             OFFSET $6::BIGINT - 1
             LIMIT 1
            "#,
            pool,
            from,
            after_at,
            after_signature.clone(),
            after_index,
            limit,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        sqlx::query_as!(
            MeteoraDammV2SwapEventRow,
            r#"
            SELECT pool_address, signature, timestamp, slot, transaction_index, event_index,
                   trade_direction, amount_a, amount_b, reserve_a_after, reserve_b_after,
                   next_sqrt_price, claiming_fee, protocol_fee, compounding_fee, referral_fee,
                   fee_token_is_a
              FROM meteora_damm_v2_swap_events
             WHERE pool_address = $1
               AND timestamp >= GREATEST($2, COALESCE($3::TIMESTAMPTZ, $2))
               AND timestamp <= COALESCE($4, 'infinity'::TIMESTAMPTZ)
               AND (timestamp < COALESCE($4, 'infinity'::TIMESTAMPTZ)
                    OR (signature, event_index) < ($5, $6))
             ORDER BY timestamp DESC, signature DESC, event_index DESC
             LIMIT $7
            "#,
            pool,
            from,
            floor,
            after_at,
            after_signature,
            after_index,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)
    }
}
