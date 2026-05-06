use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{
    Cursor, Page, RepositoryError, RepositoryResult,
    domain::{Pool, PoolCursor, PoolRepository, Protocol},
};

use crate::repository_utils::{convert_string_to_pubkey, map_sqlx_error};

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

    async fn find_paginated(
        &self,
        cursor: Option<PoolCursor>,
        limit: i64,
    ) -> RepositoryResult<Page<Pool>> {
        // Defensive clamp: callers should validate, but the repo is the
        // last line of defense before the DB.
        let limit = limit.clamp(1, MAX_PAGE_SIZE);

        // Two SQL paths to keep both branches simple and statically
        // checkable by sqlx. Each branch maps its rows to `Vec<Pool>`
        // before merging back, because `sqlx::query!` generates a
        // distinct anonymous struct per call site — the `match` arms
        // would otherwise have incompatible types.
        //
        // Cursor predicate: lexicographic ordering on
        // (first_seen_at DESC, pool_address ASC):
        //
        //   first_seen_at  <  cursor.first_seen_at
        //   OR (first_seen_at = cursor.first_seen_at
        //       AND pool_address > cursor.pool_address)
        //
        // The strict inequality on the first column is what makes the
        // pagination skip the cursor row itself.
        let pools: Vec<Pool> = match cursor {
            None => sqlx::query!(
                r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   first_seen_at, last_seen_at
            FROM pools
            ORDER BY first_seen_at DESC, pool_address ASC
            LIMIT $1
            "#,
                limit
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

            Some(cursor) => sqlx::query!(
                r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   first_seen_at, last_seen_at
            FROM pools
            WHERE first_seen_at < $1
               OR (first_seen_at = $1 AND pool_address > $2)
            ORDER BY first_seen_at DESC, pool_address ASC
            LIMIT $3
            "#,
                cursor.first_seen_at,
                cursor.pool_address.to_string(),
                limit
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

        // Build the next cursor only when the page is full.
        let next_cursor = if pools.len() as i64 >= limit {
            pools.last().map(|p| {
                Cursor::Pool(PoolCursor {
                    first_seen_at: p.first_seen_at,
                    pool_address: p.pool_address,
                })
            })
        } else {
            None
        };

        Ok(Page {
            items: pools,
            next_cursor,
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
