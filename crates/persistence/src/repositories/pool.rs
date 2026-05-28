mod query;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    Cursor, Page, PageDirection, PagePosition, PoolSort, RepositoryError, RepositoryResult,
    domain::{Pool, PoolCursor, PoolRepository, Protocol},
};

use crate::repositories::tool::{QueryMode, resolve_query_mode};
use crate::repository_utils::{convert_string_to_pubkey, map_sqlx_error};

use query::{PaginatedPoolsQuery, PoolRow, build};

pub struct PgPoolRepository {
    pool: PgPool,
}

impl PgPoolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

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
            r#"UPDATE pools SET last_seen_at = NOW() WHERE pool_address = $1"#,
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
        direction: PageDirection,
        position: Option<PagePosition>,
        sort: PoolSort,
        search: Option<String>,
        limit: i64,
    ) -> RepositoryResult<Page<Pool>> {
        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1;

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_sort_value, cursor_pool_address) = match active_cursor {
            Some(c) => (Some(c.sort_value), Some(c.pool_address.to_string())),
            None => (None, None),
        };

        // Build the dynamic query (ORDER BY + keyset + search) and run
        // it. Mapping goes through PoolRow (FromRow) then row_to_pool.
        let mut qb = build(PaginatedPoolsQuery {
            mode,
            sort,
            cursor_sort_value,
            cursor_pool_address,
            search,
            fetch_limit,
        });

        let rows: Vec<PoolRow> = qb
            .build_query_as::<PoolRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        let mut pools: Vec<Pool> = rows
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
            .collect::<Result<_, _>>()?;

        let has_more = pools.len() as i64 > effective_limit;
        if has_more {
            pools.truncate(effective_limit as usize);
        }

        if matches!(mode, QueryMode::Backward) {
            pools.reverse();
        }

        let (is_first, is_last) = match mode {
            QueryMode::Forward => (!had_cursor, !has_more),
            QueryMode::Backward => (!has_more, !had_cursor),
        };

        // Cursor construction now stamps the sort column so the next
        // request can be validated against the active sort.
        let sort_column = sort.column();
        let cursor_for = |p: &Pool| -> Cursor {
            let sort_value = match sort_column {
                yog_core::PoolSortColumn::FirstSeen => p.first_seen_at,
                yog_core::PoolSortColumn::LastSeen => p.last_seen_at,
            };
            Cursor::Pool(PoolCursor {
                sort_column,
                sort_value,
                pool_address: p.pool_address,
            })
        };

        let (prev_cursor, next_cursor) = if pools.is_empty() {
            (None, None)
        } else {
            let prev = if is_first {
                None
            } else {
                pools.first().map(cursor_for)
            };
            let next = if is_last {
                None
            } else {
                pools.last().map(cursor_for)
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
