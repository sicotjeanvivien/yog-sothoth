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
    Cursor, Page, PoolSortColumn, RepositoryError, RepositoryResult,
    domain::{
        FeeTier, Pool, PoolAccountProperties, PoolAccountResolver, PoolCatalog, PoolCounts,
        PoolCursor, PoolListQuery, PoolRepository,
    },
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

/// How many fee tiers the filter offers. The dozen-or-so real tiers hold the
/// vast majority of pools; capping at the most common keeps the dropdown short
/// and drops the long tail of one-off dynamic-fee/launch values.
const FEE_TIER_LIMIT: i64 = 8;

/// Convert a domain `fee_bps` (`rust_decimal::Decimal`) to the `BigDecimal`
/// that NUMERIC binds to at the persistence boundary. Round-trips through the
/// exact decimal string — never lossy for the small fee values we store.
fn fee_bps_to_numeric(fee_bps: rust_decimal::Decimal) -> RepositoryResult<sqlx::types::BigDecimal> {
    sqlx::types::BigDecimal::from_str(&fee_bps.to_string())
        .map_err(|e| RepositoryError::Integrity(format!("invalid fee_bps decimal: {e}")))
}

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
        let fee_bps = fee_bps_to_numeric(fee_bps)?;
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
}

#[async_trait]
impl PoolCatalog for PgPoolRepository {
    async fn find_by_address(&self, pool_address: &Pubkey) -> RepositoryResult<Option<Pool>> {
        let row = sqlx::query_as!(
            PoolRow,
            r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   fee_bps AS "fee_bps?: rust_decimal::Decimal",
                   protocol_fee_percent, partner_fee_percent, referral_fee_percent,
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

    async fn counts(&self) -> RepositoryResult<PoolCounts> {
        let row = sqlx::query!(
            r#"
            SELECT
                COUNT(*) AS "observed!",
                COUNT(*) FILTER (
                    WHERE first_seen_at > NOW() - INTERVAL '24 hours'
                ) AS "discovered_24h!"
            FROM pools
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(PoolCounts {
            observed: row.observed,
            discovered_24h: row.discovered_24h,
        })
    }

    async fn find_by_addresses(&self, pool_addresses: &[Pubkey]) -> RepositoryResult<Vec<Pool>> {
        let addresses: Vec<String> = pool_addresses.iter().map(|p| p.to_string()).collect();
        let rows = sqlx::query_as!(
            PoolRow,
            r#"
            SELECT pool_address, protocol, token_a_mint, token_b_mint,
                   fee_bps AS "fee_bps?: rust_decimal::Decimal",
                   protocol_fee_percent, partner_fee_percent, referral_fee_percent,
                   first_seen_at, last_seen_at
            FROM pools
            WHERE pool_address = ANY($1::TEXT[])
            "#,
            &addresses
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        rows.into_iter().map(Pool::try_from).collect()
    }

    async fn find_paginated(&self, query: PoolListQuery) -> RepositoryResult<Page<Pool>> {
        let PoolListQuery {
            cursor,
            direction,
            position,
            sort,
            search,
            fee_bps,
            limit,
        } = query;

        let effective_limit = limit.clamp(1, MAX_PAGE_SIZE);
        let fetch_limit = effective_limit + 1;

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();
        let (cursor_sort_value, cursor_pool_address) = match active_cursor {
            Some(c) => (Some(c.sort_value), Some(c.pool_address.to_string())),
            None => (None, None),
        };

        // NUMERIC binds to BigDecimal at the persistence boundary — same
        // lossless string round-trip as the write path.
        let fee_bps = fee_bps.map(fee_bps_to_numeric).transpose()?;

        // Build the dynamic query (ORDER BY + keyset + search + fee) and
        // run it. Mapping goes through PoolRow (FromRow) then Pool::try_from.
        let mut qb = build(PaginatedPoolsQuery {
            mode,
            sort,
            cursor_sort_value,
            cursor_pool_address,
            search,
            fee_bps,
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

    async fn list_fee_tiers(&self) -> RepositoryResult<Vec<FeeTier>> {
        // Rank tiers by pool count and keep the top N (the observed fee
        // distribution is long-tailed — a few real tiers plus a long tail of
        // one-off dynamic-fee/launch values), then re-order the survivors
        // ascending by fee for natural display. The count tie-breaks by fee
        // so the cut is deterministic.
        let rows = sqlx::query!(
            r#"
            SELECT fee_bps AS "fee_bps!: rust_decimal::Decimal", pool_count AS "pool_count!"
            FROM (
                SELECT fee_bps, COUNT(*) AS pool_count
                FROM pools
                WHERE fee_bps IS NOT NULL
                GROUP BY fee_bps
                ORDER BY pool_count DESC, fee_bps
                LIMIT $1
            ) top
            ORDER BY fee_bps
            "#,
            FEE_TIER_LIMIT,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        Ok(rows
            .into_iter()
            .map(|r| FeeTier {
                fee_bps: r.fee_bps,
                pool_count: r.pool_count,
            })
            .collect())
    }
}

#[async_trait]
impl PoolAccountResolver for PgPoolRepository {
    async fn list_unresolved(&self, limit: i64) -> RepositoryResult<Vec<Pubkey>> {
        let rows = sqlx::query!(
            r#"
            SELECT pool_address
            FROM pools
            WHERE token_a_mint IS NULL OR token_b_mint IS NULL OR fee_bps IS NULL
               OR protocol_fee_percent IS NULL OR partner_fee_percent IS NULL
               OR referral_fee_percent IS NULL
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

    async fn set_pool_account(
        &self,
        pool_address: &Pubkey,
        properties: &PoolAccountProperties,
    ) -> RepositoryResult<()> {
        let fee_bps = fee_bps_to_numeric(properties.fee_bps)?;
        // u8 → i16 (SMALLINT) is always lossless.
        sqlx::query!(
            r#"
            UPDATE pools
            SET token_a_mint = $2, token_b_mint = $3, fee_bps = $4,
                protocol_fee_percent = $5, partner_fee_percent = $6,
                referral_fee_percent = $7
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
            properties.token_a_mint.to_string(),
            properties.token_b_mint.to_string(),
            fee_bps,
            i16::from(properties.protocol_fee_percent),
            i16::from(properties.partner_fee_percent),
            i16::from(properties.referral_fee_percent),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }
}
