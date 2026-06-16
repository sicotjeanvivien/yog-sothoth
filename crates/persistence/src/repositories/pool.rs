mod query;
mod rows;

use crate::repositories::helper::convert_string_to_pubkey;
use crate::repositories::helper::{PageBuilder, map_sqlx_error, resolve_query_mode};
use async_trait::async_trait;
use query::{PaginatedPoolsQuery, build};
use rows::PoolRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;
use yog_core::{
    Cursor, Page, PageDirection, PagePosition, PoolSort, PoolSortColumn, RepositoryError,
    RepositoryResult,
    domain::{Pool, PoolCursor, PoolMintResolver, PoolRepository},
};

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
            pool.token_a_mint.map(|m| m.to_string()),
            pool.token_b_mint.map(|m| m.to_string()),
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

    async fn set_fee_bps(
        &self,
        pool_address: &Pubkey,
        fee_bps: rust_decimal::Decimal,
    ) -> RepositoryResult<()> {
        // NUMERIC binds to BigDecimal at the persistence boundary (the crate
        // also enables sqlx's bigdecimal feature for the lossless u128 columns).
        // Round-trip through the exact decimal string — never lossy for the
        // small fee values we store.
        let fee_bps = sqlx::types::BigDecimal::from_str(&fee_bps.to_string())
            .map_err(|e| RepositoryError::Integrity(format!("invalid fee_bps decimal: {e}")))?;
        sqlx::query!(
            r#"UPDATE pools SET fee_bps = $2 WHERE pool_address = $1"#,
            pool_address.to_string(),
            fee_bps,
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    async fn find_by_address(&self, pool_address: &Pubkey) -> RepositoryResult<Option<Pool>> {
        let row = sqlx::query_as!(
            PoolRow,
            r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   fee_bps AS "fee_bps?: rust_decimal::Decimal",
                   first_seen_at, last_seen_at
            FROM pools
            WHERE pool_address = $1
            "#,
            pool_address.to_string()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        row.map(Pool::try_from).transpose()
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
        // it. Mapping goes through PoolRow (FromRow) then Pool::try_from.
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

        let pools: Vec<Pool> = rows
            .into_iter()
            .map(Pool::try_from)
            .collect::<Result<_, _>>()?;

        let sort_column = sort.column();

        Ok(
            PageBuilder::new(pools, effective_limit, mode, had_cursor).finalize(|p| {
                let sort_value = match sort_column {
                    PoolSortColumn::FirstSeen => p.first_seen_at,
                    PoolSortColumn::LastSeen => p.last_seen_at,
                };

                Cursor::Pool(PoolCursor {
                    sort_column,
                    sort_value,
                    pool_address: p.pool_address,
                })
            }),
        )
    }
}

#[async_trait]
impl PoolMintResolver for PgPoolRepository {
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address
            FROM pools
            WHERE token_a_mint IS NULL OR token_b_mint IS NULL
            ORDER BY first_seen_at
            LIMIT $1
            "#,
            limit,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter()
            .map(|r| convert_string_to_pubkey(r.pool_address, "pool_address"))
            .collect()
    }

    async fn set_mints(
        &self,
        pool_address: &Pubkey,
        token_a_mint: &Pubkey,
        token_b_mint: &Pubkey,
    ) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            UPDATE pools
            SET token_a_mint = $2, token_b_mint = $3
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
            token_a_mint.to_string(),
            token_b_mint.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}
