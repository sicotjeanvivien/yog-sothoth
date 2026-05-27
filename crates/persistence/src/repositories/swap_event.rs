use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{Protocol, SwapCursor, SwapEvent, SwapEventRepository, TradeDirection},
    tools::{Cursor, Page, PageDirection, PagePosition},
};

use crate::{
    repositories::tool::{QueryMode, resolve_query_mode},
    repository_utils::{
        convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey,
        convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error,
    },
};

/// Maximum number of rows returned in a single page, regardless of the
/// caller's `limit`. Acts as a backstop against an oversized query if
/// the API-layer validation is bypassed.
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
            event.signature,
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
            Some(c) => (Some(c.timestamp), Some(c.signature)),
            None => (None, None),
        };

        // Two SQL paths — one per traversal mode. Each branch maps to
        // Vec<SwapEvent> in place because sqlx generates a distinct
        // anonymous Record struct per query! invocation, which
        // prevents merging the rows in a single Vec after the match.
        //
        // Forward mode: order DESC, predicate "strictly older than
        // cursor OR same timestamp with later signature".
        // Backward mode: order ASC, predicate "strictly newer than
        // cursor OR same timestamp with earlier signature". The
        // result is reversed below before returning so the caller
        // always observes natural display order.
        let mut events: Vec<SwapEvent> = match mode {
            QueryMode::Forward => sqlx::query!(
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
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(|row| {
                Ok(SwapEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                    token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
                    token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
                    trade_direction: TradeDirection::from_str(&row.trade_direction).map_err(
                        |_| {
                            RepositoryError::Integrity(format!(
                                "invalid trade_direction: {}",
                                row.trade_direction
                            ))
                        },
                    )?,
                    amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
                    amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
                    reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
                    reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
                    next_sqrt_price: convert_bigdecimal_to_u128(
                        row.next_sqrt_price,
                        "next_sqrt_price",
                    )?,
                    claiming_fee: convert_i64_to_u64(row.claiming_fee, "claiming_fee")?,
                    protocol_fee: convert_i64_to_u64(row.protocol_fee, "protocol_fee")?,
                    compounding_fee: convert_i64_to_u64(row.compounding_fee, "compounding_fee")?,
                    referral_fee: convert_i64_to_u64(row.referral_fee, "referral_fee")?,
                    fee_token_is_a: row.fee_token_is_a,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()?,

            QueryMode::Backward => sqlx::query!(
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
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(|row| {
                Ok(SwapEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                    token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
                    token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
                    trade_direction: TradeDirection::from_str(&row.trade_direction).map_err(
                        |_| {
                            RepositoryError::Integrity(format!(
                                "invalid trade_direction: {}",
                                row.trade_direction
                            ))
                        },
                    )?,
                    amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
                    amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
                    reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
                    reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
                    next_sqrt_price: convert_bigdecimal_to_u128(
                        row.next_sqrt_price,
                        "next_sqrt_price",
                    )?,
                    claiming_fee: convert_i64_to_u64(row.claiming_fee, "claiming_fee")?,
                    protocol_fee: convert_i64_to_u64(row.protocol_fee, "protocol_fee")?,
                    compounding_fee: convert_i64_to_u64(row.compounding_fee, "compounding_fee")?,
                    referral_fee: convert_i64_to_u64(row.referral_fee, "referral_fee")?,
                    fee_token_is_a: row.fee_token_is_a,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()?,
        };

        // Peek N+1 outcome.
        let has_more = events.len() as i64 > effective_limit;
        if has_more {
            events.truncate(effective_limit as usize);
        }

        // Restore natural display order for backward queries.
        if matches!(mode, QueryMode::Backward) {
            events.reverse();
        }

        // Compute boundary flags.
        //
        // Forward query: has_more means more rows exist further
        // (older events) → is_last = false. is_first inferred from
        // "no cursor" → we started at the natural top.
        //
        // Backward query: has_more means more rows exist on the
        // newer side → is_first = false. is_last inferred from
        // "no cursor" → we jumped to or were already at the bottom.
        let (is_first, is_last) = match mode {
            QueryMode::Forward => (!had_cursor, !has_more),
            QueryMode::Backward => (!has_more, !had_cursor),
        };

        // Empty page: both boundaries simultaneously.
        let (prev_cursor, next_cursor) = if events.is_empty() {
            (None, None)
        } else {
            let prev = if is_first {
                None
            } else {
                events.first().map(|e| {
                    Cursor::Swap(SwapCursor {
                        timestamp: e.timestamp,
                        signature: e.signature.clone(),
                    })
                })
            };
            let next = if is_last {
                None
            } else {
                events.last().map(|e| {
                    Cursor::Swap(SwapCursor {
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
