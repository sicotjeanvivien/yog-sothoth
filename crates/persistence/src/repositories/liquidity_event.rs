use async_trait::async_trait;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    RepositoryError, RepositoryResult,
    domain::{LiquidityCursor, LiquidityEvent, LiquidityEventRepository, Protocol},
    tools::{Cursor, Page},
};

use crate::repository_utils::{
    convert_bigdecimal_to_u128, convert_i64_to_u64, convert_string_to_pubkey, convert_u64_to_i64,
    convert_u128_to_bigdecimal, map_sqlx_error, parse_string_to_liquidity_event_kind,
};

/// Maximum number of rows returned in a single page, regardless of the
/// caller's `limit`. Acts as a backstop against an oversized query if
/// the API-layer validation is bypassed.
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

    /// Paginate liquidity events for a pool by keyset on
    /// `(timestamp, signature)`.
    ///
    /// Ordering is `timestamp DESC, signature ASC`. The cursor points to
    /// the last item of the previous page; the WHERE clause uses
    /// lexicographic-style comparison to fetch the strictly-next slice.
    async fn find_by_pool_paginated(
        &self,
        pool_address: &Pubkey,
        cursor: Option<LiquidityCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<LiquidityEvent>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);

        let (cursor_timestamp, cursor_signature) = match cursor {
            Some(c) => (Some(c.timestamp), Some(c.signature)),
            None => (None, None),
        };

        let rows = sqlx::query!(
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
            effective_limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        let events = rows
            .into_iter()
            .map(|row| {
                Ok(LiquidityEvent {
                    pool_address: convert_string_to_pubkey(row.pool_address, "pool_address")?,
                    protocol: Protocol::from_str(&row.protocol).map_err(|e| {
                        RepositoryError::Integrity(format!("invalid protocol: {e}"))
                    })?,
                    signature: row.signature,
                    timestamp: row.timestamp,
                    token_a_mint: convert_string_to_pubkey(row.token_a_mint, "token_a_mint")?,
                    token_b_mint: convert_string_to_pubkey(row.token_b_mint, "token_b_mint")?,
                    liquidity_event_kind: parse_string_to_liquidity_event_kind(
                        row.liquidity_event_kind,
                        "liquidity_event_kind",
                    )?,
                    amount_a: convert_i64_to_u64(row.amount_a, "amount_a")?,
                    amount_b: convert_i64_to_u64(row.amount_b, "amount_b")?,
                    liquidity_delta: convert_bigdecimal_to_u128(
                        row.liquidity_delta,
                        "liquidity_delta",
                    )?,
                    reserve_a_after: convert_i64_to_u64(row.reserve_a_after, "reserve_a_after")?,
                    reserve_b_after: convert_i64_to_u64(row.reserve_b_after, "reserve_b_after")?,
                    position: convert_string_to_pubkey(row.position, "position")?,
                    owner: convert_string_to_pubkey(row.owner, "owner")?,
                })
            })
            .collect::<RepositoryResult<Vec<_>>>()?;

        Ok(Page::build(events, effective_limit as usize, |last| {
            Cursor::Liquidity(LiquidityCursor {
                timestamp: last.timestamp,
                signature: last.signature.clone(),
            })
        }))
    }
}
