//! Liquidity events repository: inserts new events and paginates
//! existing ones by pool.
//!
//! Both reads are static SQL (one query per traversal mode); the row
//! shape and the mapping to domain are shared. No dynamic SQL here,
//! so the layout stays single-file: `LiquidityEventRow` and its
//! `TryFrom` to domain live at the bottom of this module.
mod rows;

use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{LiquidityCursor, LiquidityEvent, LiquidityEventRepository},
    tools::{Cursor, Page, PageDirection, PagePosition},
};

use crate::{
    repositories::{
        liquidity_event::rows::LiquidityEventRow,
        tool::{QueryMode, resolve_query_mode},
    },
    repository_utils::{convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error},
};

/// Maximum number of rows returned in a single page, regardless of the
/// caller's `limit`. Backstop against an oversized query if the
/// API-layer validation is bypassed.
const MAX_PAGE_SIZE: i64 = 200;

pub struct PgLiquidityEventRepository {
    pool: PgPool,
}

impl PgLiquidityEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LiquidityEventRepository for PgLiquidityEventRepository {
    async fn insert(&self, event: &LiquidityEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO liquidity_events (
                pool_address, protocol, signature,
                token_a_mint, token_b_mint,
                liquidity_event_kind, amount_a, amount_b, liquidity_delta,
                reserve_a_after, reserve_b_after,
                position, owner,
                timestamp
            )
            VALUES (
                $1, $2, $3,
                $4, $5,
                $6, $7, $8, $9,
                $10, $11,
                $12, $13,
                $14
            )
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.protocol.as_str(),
            event.signature,
            event.token_a_mint.to_string(),
            event.token_b_mint.to_string(),
            event.liquidity_event_kind.as_str(),
            convert_u64_to_i64(event.amount_a, "amount_a")?,
            convert_u64_to_i64(event.amount_b, "amount_b")?,
            convert_u128_to_bigdecimal(event.liquidity_delta, "liquidity_delta"),
            convert_u64_to_i64(event.reserve_a_after, "reserve_a_after")?,
            convert_u64_to_i64(event.reserve_b_after, "reserve_b_after")?,
            event.position.to_string(),
            event.owner.to_string(),
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    /// Paginate liquidity events for a pool with bidirectional navigation.
    ///
    /// See `PgSwapEventRepository::find_by_pool_paginated` for the full
    /// design notes — the implementation is structurally identical, only
    /// the row mapping differs.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<LiquidityCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityEvent>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1;

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_timestamp, cursor_signature) = match active_cursor {
            Some(c) => (Some(c.timestamp), Some(c.signature)),
            None => (None, None),
        };

        // Two static SQL paths — one per traversal mode. Both produce
        // Vec<LiquidityEventRow>; the mapping to domain runs once after
        // the match, no duplication.
        let rows: Vec<LiquidityEventRow> = match mode {
            QueryMode::Forward => sqlx::query_as!(
                LiquidityEventRow,
                r#"
                SELECT pool_address, protocol, signature,
                       token_a_mint, token_b_mint,
                       liquidity_event_kind, amount_a, amount_b, liquidity_delta,
                       reserve_a_after, reserve_b_after,
                       position, owner,
                       timestamp
                FROM liquidity_events
                WHERE pool_address = $1
                  AND (
                      $2::TIMESTAMPTZ IS NULL
                      OR timestamp < $2
                      OR (timestamp = $2 AND signature > $3)
                  )
                ORDER BY timestamp DESC, signature ASC
                LIMIT $4
                "#,
                pool_address.to_string(),
                cursor_timestamp,
                cursor_signature,
                fetch_limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,

            QueryMode::Backward => sqlx::query_as!(
                LiquidityEventRow,
                r#"
                SELECT pool_address, protocol, signature,
                       token_a_mint, token_b_mint,
                       liquidity_event_kind, amount_a, amount_b, liquidity_delta,
                       reserve_a_after, reserve_b_after,
                       position, owner,
                       timestamp
                FROM liquidity_events
                WHERE pool_address = $1
                  AND (
                      $2::TIMESTAMPTZ IS NULL
                      OR timestamp > $2
                      OR (timestamp = $2 AND signature < $3)
                  )
                ORDER BY timestamp ASC, signature DESC
                LIMIT $4
                "#,
                pool_address.to_string(),
                cursor_timestamp,
                cursor_signature,
                fetch_limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?,
        };

        let mut events: Vec<LiquidityEvent> = rows
            .into_iter()
            .map(LiquidityEvent::try_from)
            .collect::<Result<_, _>>()?;

        let has_more = events.len() as i64 > effective_limit;
        if has_more {
            events.truncate(effective_limit as usize);
        }

        if matches!(mode, QueryMode::Backward) {
            events.reverse();
        }

        let (is_first, is_last) = match mode {
            QueryMode::Forward => (!had_cursor, !has_more),
            QueryMode::Backward => (!has_more, !had_cursor),
        };

        let (prev_cursor, next_cursor) = if events.is_empty() {
            (None, None)
        } else {
            let prev = if is_first {
                None
            } else {
                events.first().map(|e| {
                    Cursor::Liquidity(LiquidityCursor {
                        timestamp: e.timestamp,
                        signature: e.signature.clone(),
                    })
                })
            };
            let next = if is_last {
                None
            } else {
                events.last().map(|e| {
                    Cursor::Liquidity(LiquidityCursor {
                        timestamp: e.timestamp,
                        signature: e.signature.clone(),
                    })
                })
            };
            (prev, next)
        };

        Ok(Page {
            items: events,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        })
    }
}
