//! DAMM v2 swap events repository: inserts new events and paginates
//! existing ones by pool.
//!
//! Both reads are static SQL (one query per traversal mode); the row
//! shape and the mapping to domain are shared. No dynamic SQL here,
//! so the layout stays single-file: `MeteoraDammV2SwapEventRow` and its
//! `TryFrom` to domain live in the sibling `rows.rs` module.

mod rows;

use crate::repositories::helper::{
    PageBuilder, QueryMode, convert_u64_to_i64, convert_u128_to_bigdecimal, map_sqlx_error,
    resolve_query_mode,
};
use async_trait::async_trait;
use rows::MeteoraDammV2SwapEventRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    RepositoryResult,
    domain::{
        MeteoraDammV2SwapEvent, MeteoraDammV2SwapEventCursor, MeteoraDammV2SwapEventRepository,
    },
    tools::{Cursor, Page, PageDirection, PagePosition},
};

/// Maximum number of rows returned in a single page, regardless of the
/// caller's `limit`. Backstop against an oversized query if the
/// API-layer validation is bypassed.
const MAX_PAGE_SIZE: i64 = 200;

pub struct PgMeteoraDammV2SwapEventRepository {
    pool: PgPool,
}

impl PgMeteoraDammV2SwapEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MeteoraDammV2SwapEventRepository for PgMeteoraDammV2SwapEventRepository {
    async fn insert(&self, event: &MeteoraDammV2SwapEvent) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO meteora_damm_v2_swap_events (
                pool_address, signature,
                token_a_mint, token_b_mint,
                trade_direction, amount_a, amount_b,
                reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee,
                fee_token_is_a,
                timestamp
            )
            VALUES (
                $1, $2,
                $3, $4,
                $5, $6, $7,
                $8, $9, $10,
                $11, $12, $13, $14,
                $15,
                $16
            )
            ON CONFLICT (signature, timestamp) DO NOTHING
            "#,
            event.pool_address.to_string(),
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

    /// Paginate DAMM v2 swap events for a pool with bidirectional navigation.
    ///
    /// Natural display order is `timestamp DESC, signature ASC` (newest
    /// first, deterministic tie-break on signature).
    ///
    /// - `cursor` + `direction` cooperate: traverse forward (older events)
    ///   or backward (newer events) from the cursor position.
    /// - `position` jumps to a list boundary (`First` = newest events,
    ///   `Last` = oldest events), ignoring any cursor.
    /// - Peek N+1 detects whether more rows exist on the queried side in
    ///   a single query.
    ///
    /// Backward queries (Prev / Last) reverse the SQL ORDER BY and the
    /// resulting vector before returning, so the caller always observes
    /// the page in display order.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<MeteoraDammV2SwapEventCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<MeteoraDammV2SwapEvent>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1;

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_timestamp, cursor_signature) = match active_cursor {
            Some(c) => (Some(c.timestamp), Some(c.signature.to_string())),
            None => (None, None),
        };

        let rows: Vec<MeteoraDammV2SwapEventRow> = match mode {
            QueryMode::Forward => sqlx::query_as!(
                MeteoraDammV2SwapEventRow,
                r#"
                SELECT pool_address, signature,
                       token_a_mint, token_b_mint,
                       trade_direction, amount_a, amount_b,
                       reserve_a_after, reserve_b_after, next_sqrt_price,
                       claiming_fee, protocol_fee, compounding_fee, referral_fee,
                       fee_token_is_a,
                       timestamp
                FROM meteora_damm_v2_swap_events
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
                MeteoraDammV2SwapEventRow,
                r#"
                SELECT pool_address, signature,
                       token_a_mint, token_b_mint,
                       trade_direction, amount_a, amount_b,
                       reserve_a_after, reserve_b_after, next_sqrt_price,
                       claiming_fee, protocol_fee, compounding_fee, referral_fee,
                       fee_token_is_a,
                       timestamp
                FROM meteora_damm_v2_swap_events
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

        let events: Vec<MeteoraDammV2SwapEvent> = rows
            .into_iter()
            .map(MeteoraDammV2SwapEvent::try_from)
            .collect::<Result<_, _>>()?;

        Ok(
            PageBuilder::new(events, effective_limit, mode, had_cursor).finalize(|e| {
                Cursor::MeteoraDammV2SwapEvent(MeteoraDammV2SwapEventCursor {
                    timestamp: e.timestamp,
                    signature: e.signature,
                })
            }),
        )
    }
}
