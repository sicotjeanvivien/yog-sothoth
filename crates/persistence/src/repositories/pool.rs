use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, PageDirection, PagePosition, RepositoryError, RepositoryResult,
    domain::{Pool, PoolCursor, PoolRepository, Protocol},
};

use crate::{
    repositories::tool::{QueryMode, resolve_query_mode},
    repository_utils::{convert_string_to_pubkey, map_sqlx_error},
};

pub struct PgPoolRepository {
    pool: PgPool,
}

impl PgPoolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Hard upper bound on page size, regardless of what the caller asks for.
/// Prevents pathological queries from slipping through API validation.
const MAX_PAGE_SIZE: i64 = 200;

#[async_trait]
impl PoolRepository for PgPoolRepository {
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO pools
                (pool_address, protocol, token_a_mint, token_b_mint,
                 first_seen_at, last_seen_at)
            VALUES ($1, $2, $3, $4, $5, $5)
            ON CONFLICT (pool_address) DO UPDATE
                SET last_seen_at = EXCLUDED.last_seen_at
            "#,
            pool.pool_address.to_string(),
            pool.protocol.as_str(),
            pool.token_a_mint.to_string(),
            pool.token_b_mint.to_string(),
            pool.last_seen_at,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn touch_last_seen(&self, pool_address: &Pubkey) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            UPDATE pools
            SET last_seen_at = NOW()
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(())
    }

    async fn find_by_address(&self, pool_address: &Pubkey) -> RepositoryResult<Option<Pool>> {
        let row = sqlx::query!(
            r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   first_seen_at, last_seen_at
            FROM pools
            WHERE pool_address = $1
            "#,
            pool_address.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(|r| {
            row_to_pool(
                r.pool_address,
                r.protocol,
                r.token_a_mint,
                r.token_b_mint,
                r.first_seen_at,
                r.last_seen_at,
            )
        })
        .transpose()
    }

    /// Paginate pools with bidirectional navigation.
    ///
    /// Natural display order is `first_seen_at DESC, pool_address ASC`
    /// (newest pools first, deterministic tie-break on address).
    ///
    /// - `cursor` + `direction` cooperate: traverse forward (older
    ///   pools) or backward (newer pools) from the cursor position.
    /// - `position` jumps to a list boundary (`First` = newest pools,
    ///   `Last` = oldest pools), ignoring any cursor.
    /// - Peek N+1 detects whether more rows exist on the queried side
    ///   in a single query.
    ///
    /// Backward queries (Prev / Last) reverse the SQL ORDER BY and the
    /// resulting vector before returning, so the caller always observes
    /// the page in display order.
    async fn find_paginated(
        &self,
        cursor: Option<PoolCursor>,
        direction: PageDirection,
        position: Option<PagePosition>,
        limit: i64,
    ) -> RepositoryResult<Page<Pool>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1; // peek N+1

        // Resolve effective query mode. `position` overrides
        // `cursor` + `direction`; the handler enforces mutual
        // exclusivity but the repo defends in depth.
        let mode = resolve_query_mode(position, &cursor, direction);

        // Cursor is meaningful only relative to a position; ignored
        // when jumping to a boundary.
        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_first_seen_at, cursor_pool_address) = match active_cursor {
            Some(c) => (Some(c.first_seen_at), Some(c.pool_address.to_string())),
            None => (None, None),
        };

        // Two SQL paths — one per traversal mode. Each maps to
        // Vec<Pool> in its own branch because sqlx generates a
        // distinct anonymous Record struct per query! invocation,
        // which prevents merging the rows in a single Vec after
        // the match.
        let mut pools: Vec<Pool> = match mode {
            QueryMode::Forward => sqlx::query!(
                r#"
                SELECT pool_address, protocol, token_a_mint, token_b_mint,
                       first_seen_at, last_seen_at
                FROM pools
                WHERE $1::TIMESTAMPTZ IS NULL
                   OR first_seen_at < $1
                   OR (first_seen_at = $1 AND pool_address > $2)
                ORDER BY first_seen_at DESC, pool_address ASC
                LIMIT $3
                "#,
                cursor_first_seen_at,
                cursor_pool_address,
                fetch_limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(|r| {
                row_to_pool(
                    r.pool_address,
                    r.protocol,
                    r.token_a_mint,
                    r.token_b_mint,
                    r.first_seen_at,
                    r.last_seen_at,
                )
            })
            .collect::<Result<_, _>>()?,

            QueryMode::Backward => sqlx::query!(
                r#"
                SELECT pool_address, protocol, token_a_mint, token_b_mint,
                       first_seen_at, last_seen_at
                FROM pools
                WHERE $1::TIMESTAMPTZ IS NULL
                   OR first_seen_at > $1
                   OR (first_seen_at = $1 AND pool_address < $2)
                ORDER BY first_seen_at ASC, pool_address DESC
                LIMIT $3
                "#,
                cursor_first_seen_at,
                cursor_pool_address,
                fetch_limit,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?
            .into_iter()
            .map(|r| {
                row_to_pool(
                    r.pool_address,
                    r.protocol,
                    r.token_a_mint,
                    r.token_b_mint,
                    r.first_seen_at,
                    r.last_seen_at,
                )
            })
            .collect::<Result<_, _>>()?,
        };

        // Peek N+1 outcome.
        let has_more = pools.len() as i64 > effective_limit;
        if has_more {
            pools.truncate(effective_limit as usize);
        }

        // Restore natural display order for backward queries.
        if matches!(mode, QueryMode::Backward) {
            pools.reverse();
        }

        // Compute boundary flags.
        //
        // Forward query: has_more means more rows exist further
        // (older pools) → is_last = false. is_first inferred from
        // "no cursor" → we started at the natural top.
        //
        // Backward query: has_more means more rows exist on the
        // newer side → is_first = false. is_last inferred from
        // "no cursor" → we jumped to the bottom (or were already there).
        let (is_first, is_last) = match mode {
            QueryMode::Forward => (!had_cursor, !has_more),
            QueryMode::Backward => (!has_more, !had_cursor),
        };

        // Empty page: both boundaries simultaneously.
        let (prev_cursor, next_cursor) = if pools.is_empty() {
            (None, None)
        } else {
            let prev = if is_first {
                None
            } else {
                pools.first().map(|p| {
                    Cursor::Pool(PoolCursor {
                        first_seen_at: p.first_seen_at,
                        pool_address: p.pool_address,
                    })
                })
            };
            let next = if is_last {
                None
            } else {
                pools.last().map(|p| {
                    Cursor::Pool(PoolCursor {
                        first_seen_at: p.first_seen_at,
                        pool_address: p.pool_address,
                    })
                })
            };
            (prev, next)
        };

        Ok(Page {
            items: pools,
            next_cursor,
            prev_cursor,
            is_first,
            is_last,
        })
    }
}

// ---- Row mapping helpers ---------------------------------------------------

/// Map a database row to a domain `Pool`.
///
/// All fields use canonical string representations:
///   - Pubkeys: base58
///   - Protocol: snake_case (see `Protocol::as_str`)
///   - Timestamps: TIMESTAMPTZ (mapped directly to `DateTime<Utc>`)
///
/// Decode failures (malformed pubkey, unknown protocol) are surfaced as
/// `RepositoryError::Integrity` — they indicate either schema corruption
/// or an out-of-sync migration, never a runtime data issue.
///
/// Takes owned `String`s because that is what `sqlx::query!` produces
/// for `TEXT` columns; passing them by value avoids needless borrows
/// at every call site.
fn row_to_pool(
    pool_address: String,
    protocol: String,
    token_a_mint: String,
    token_b_mint: String,
    first_seen_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
) -> RepositoryResult<Pool> {
    Ok(Pool {
        pool_address: convert_string_to_pubkey(pool_address, "pool_address")?,
        protocol: Protocol::from_str(&protocol)
            .map_err(|e| RepositoryError::Integrity(format!("invalid protocol: {e}")))?,
        token_a_mint: convert_string_to_pubkey(token_a_mint, "token_a_mint")?,
        token_b_mint: convert_string_to_pubkey(token_b_mint, "token_b_mint")?,
        first_seen_at,
        last_seen_at,
    })
}
