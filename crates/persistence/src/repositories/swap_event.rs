//! Swap events repository: inserts new events and paginates existing
//! ones by pool.
//!
//! Both reads are static SQL (one query per traversal mode); the row
//! shape and the mapping to domain are shared. No dynamic SQL here,
//! so the layout stays single-file: `SwapEventRow` and its `TryFrom`
//! to domain live at the bottom of this module.
mod rows;

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error,
    resolve_query_mode,
};
use async_trait::async_trait;
use rows::SwapEventRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{SwapCursor, SwapEvent, SwapEventRepository},
    tools::{Cursor, Page, PageDirection, PagePosition},
};

/// Maximum number of rows returned in a single page, regardless of the
/// caller's `limit`. Backstop against an oversized query if the
/// API-layer validation is bypassed.
const MAX_PAGE_SIZE: i64 = 200;

pub struct PgSwapEventRepository {
    pool: PgPool,
}

impl PgSwapEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SwapEventRepository for PgSwapEventRepository {
    async fn insert(&self, event: &SwapEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO swap_events (
                pool_address, protocol, signature,
                token_a_mint, token_b_mint,
                trade_direction, amount_a, amount_b,
                reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee,
                fee_token_is_a,
                timestamp
            )
            VALUES (
                $1, $2, $3,
                $4, $5,
                $6, $7, $8,
                $9, $10, $11,
                $12, $13, $14, $15,
                $16,
                $17
            )
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
            event.protocol.as_str(),
            event.signature.to_string(),
            event.token_a_mint.to_string(),
            event.token_b_mint.to_string(),
            event.trade_direction.as_str(),
            convert_u64_to_i64(event.amount_a, "amount_a")?,
            convert_u64_to_i64(event.amount_b, "amount_b")?,
            convert_u64_to_i64(event.reserve_a_after, "reserve_a_after")?,
            convert_u64_to_i64(event.reserve_b_after, "reserve_b_after")?,
            convert_u128_to_bigdecimal(event.next_sqrt_price, "next_sqrt_price"),
            convert_u64_to_i64(event.claiming_fee, "claiming_fee")?,
            convert_u64_to_i64(event.protocol_fee, "protocol_fee")?,
            convert_u64_to_i64(event.compounding_fee, "compounding_fee")?,
            convert_u64_to_i64(event.referral_fee, "referral_fee")?,
            event.fee_token_is_a,
            event.timestamp,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    /// Paginate swap events for a pool with bidirectional navigation.
    ///
    /// Natural display order is `timestamp DESC, signature ASC` (newest
    /// first, deterministic tie-break on signature).
    ///
    /// - `cursor` + `direction` cooperate: traverse forward (older
    ///   events) or backward (newer events) from the cursor position.
    /// - `position` jumps to a list boundary (`First` = newest events,
    ///   `Last` = oldest events), ignoring any cursor.
    /// - Peek N+1 detects whether more rows exist on the queried side
    ///   in a single query.
    ///
    /// Backward queries (Prev / Last) reverse the SQL ORDER BY and the
    /// resulting vector before returning, so the caller always observes
    /// the page in display order.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<SwapCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<SwapEvent>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1; // peek N+1

        let mode = resolve_query_mode(position, &cursor, direction);

        // Cursor is meaningful only relative to a position; ignored
        // when jumping to a boundary.
        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_timestamp, cursor_signature) = match active_cursor {
            Some(c) => (Some(c.timestamp), Some(c.signature.to_string())),
            None => (None, None),
        };

        // Two static SQL paths — one per traversal mode. Both produce
        // Vec<SwapEventRow>; the mapping to domain runs once after the
        // match, no duplication.
        //
        // Forward mode: order DESC, predicate "strictly older than
        // cursor OR same timestamp with later signature".
        // Backward mode: order ASC, predicate "strictly newer than
        // cursor OR same timestamp with earlier signature". The
        // result is reversed below before returning so the caller
        // always observes natural display order.
        let rows: Vec<SwapEventRow> = match mode {
            QueryMode::Forward => sqlx::query_as!(
                SwapEventRow,
                r#"
                SELECT pool_address, protocol, signature,
                       token_a_mint, token_b_mint,
                       trade_direction, amount_a, amount_b,
                       reserve_a_after, reserve_b_after, next_sqrt_price,
                       claiming_fee, protocol_fee, compounding_fee, referral_fee,
                       fee_token_is_a,
                       timestamp
                FROM swap_events
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
                SwapEventRow,
                r#"
                SELECT pool_address, protocol, signature,
                       token_a_mint, token_b_mint,
                       trade_direction, amount_a, amount_b,
                       reserve_a_after, reserve_b_after, next_sqrt_price,
                       claiming_fee, protocol_fee, compounding_fee, referral_fee,
                       fee_token_is_a,
                       timestamp
                FROM swap_events
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

        let events: Vec<SwapEvent> = rows
            .into_iter()
            .map(SwapEvent::try_from)
            .collect::<Result<_, _>>()?;

        Ok(
            PageBuilder::new(events, effective_limit, mode, had_cursor).finalize(|e| {
                Cursor::Swap(SwapCursor {
                    timestamp: e.timestamp,
                    signature: e.signature,
                })
            }),
        )
    }
}
