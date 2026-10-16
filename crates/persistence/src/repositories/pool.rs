mod query;
mod rows;

use crate::repositories::helper::{PageBuilder, map_sqlx_error, resolve_query_mode};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use query::{PaginatedPoolsQuery, TouchedSinceQuery, build, build_touched_since_count};
use rows::PoolRow;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use yog_core::{
    Cursor, PoolSortColumn, RepositoryError, RepositoryResult,
    domain::{
        FeeTier, Pool, PoolCatalog, PoolCounts, PoolCursor, PoolListQuery, PoolPage,
        PoolRegistryProperties, PoolRepository,
    },
};

pub struct PgPoolRepository {
    pool: PgPool,
}

impl PgPoolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// How many pools matching the same filters were touched after the
    /// traversal's fence — see `PoolPage::touched_since`. `0` without a query
    /// when it is knowably nothing: an unfenced sort, or a first page.
    async fn touched_since(
        &self,
        as_of: Option<DateTime<Utc>>,
        had_cursor: bool,
        search: Option<String>,
        fee_bps: Option<BigDecimal>,
    ) -> RepositoryResult<i64> {
        let Some(as_of) = as_of.filter(|_| had_cursor) else {
            return Ok(0);
        };

        let mut qb = build_touched_since_count(TouchedSinceQuery {
            as_of,
            search,
            fee_bps,
        });

        qb.build_query_scalar::<i64>()
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error)
    }
}

const MAX_PAGE_SIZE: i64 = 200;

/// How stale a cursor's snapshot fence may be before the traversal re-anchors.
/// Beyond it the cursor is a bookmark, not a reader mid-scroll: replaying it
/// serves a past snapshot while `touched_since` counts most of the table.
const MAX_FENCE_AGE: chrono::Duration = chrono::Duration::hours(1);

/// The anchor a traversal runs under, once the cursor's claim has been vetted.
struct ResolvedFence {
    as_of: DateTime<Utc>,
    /// Whether the cursor that made the claim may still anchor the keyset.
    /// ⚠️ A refused claim takes the cursor with it: a fresh anchor over a stale
    /// `sort_value` serves again a pool already shown and touched since.
    keeps_cursor: bool,
}

/// Vets the fence a cursor claims — unsigned, editable client input. Refused in
/// the future (`last_seen_at <= as_of` would match everything) and past
/// [`MAX_FENCE_AGE`]. A cursor claiming nothing predates the field and keeps its place.
fn resolve_fence(claimed: Option<DateTime<Utc>>, now: DateTime<Utc>) -> ResolvedFence {
    match claimed {
        Some(as_of) if as_of <= now && now - as_of <= MAX_FENCE_AGE => ResolvedFence {
            as_of,
            keeps_cursor: true,
        },
        // Nothing claimed: mint an anchor, but the cursor itself is sound.
        None => ResolvedFence {
            as_of: now,
            keeps_cursor: true,
        },
        Some(_) => ResolvedFence {
            as_of: now,
            keeps_cursor: false,
        },
    }
}

/// How many fee tiers the filter offers, the most common ones: the long tail
/// of one-off dynamic-fee and launch values is dropped.
const FEE_TIER_LIMIT: i64 = 8;

/// Converts a domain `fee_bps` to the `BigDecimal` NUMERIC binds to, through
/// its exact decimal string.
pub(super) fn fee_bps_to_numeric(
    fee_bps: rust_decimal::Decimal,
) -> RepositoryResult<sqlx::types::BigDecimal> {
    sqlx::types::BigDecimal::from_str(&fee_bps.to_string())
        .map_err(|e| RepositoryError::Integrity(format!("invalid fee_bps decimal: {e}")))
}

#[async_trait]
impl PoolRepository for PgPoolRepository {
    /// Keeps the later of the stored and incoming `last_seen_at`, both clamped to `NOW()`.
    /// ⚠️ The listing's fence rests on the column never decreasing; without the clamp, a value
    /// ahead of the database clock would stay forever and hide the pool from every fenced page.
    async fn upsert(&self, pool: &Pool) -> RepositoryResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO pools
                (pool_address, protocol, token_a_mint, token_b_mint,
                 first_seen_at, last_seen_at)
            VALUES ($1, $2, $3, $4, LEAST($5, NOW()), LEAST($5, NOW()))
            ON CONFLICT (pool_address) DO UPDATE
                SET last_seen_at = GREATEST(
                        LEAST(pools.last_seen_at, NOW()),
                        EXCLUDED.last_seen_at
                    )
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

    /// Same rule as [`Self::upsert`], collapsed: with `NOW()` as the incoming
    /// value, `GREATEST(LEAST(last_seen_at, NOW()), NOW())` is `NOW()`.
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

    async fn mark_needs_refresh(&self, pool_address: &Pubkey) -> RepositoryResult<()> {
        sqlx::query!(
            r#"UPDATE pools SET needs_refresh = TRUE WHERE pool_address = $1"#,
            pool_address.to_string(),
        )
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;
        Ok(())
    }

    /// One statement, so the properties and the flag can never disagree: either
    /// the refreshed values and the lowered flag both commit, or neither does
    /// and the pool is proposed again next cycle.
    async fn set_registry_properties(
        &self,
        pool_address: &Pubkey,
        core: &PoolRegistryProperties,
    ) -> RepositoryResult<()> {
        let fee_bps = fee_bps_to_numeric(core.fee_bps)?;
        sqlx::query!(
            r#"
            UPDATE pools
            SET token_a_mint  = $2,
                token_b_mint  = $3,
                fee_bps       = $4,
                needs_refresh = FALSE
            WHERE pool_address = $1
            "#,
            pool_address.to_string(),
            core.token_a_mint.to_string(),
            core.token_b_mint.to_string(),
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

    async fn find_paginated(&self, query: PoolListQuery) -> RepositoryResult<PoolPage> {
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

        let sort_column = sort.column();

        // Minted here, where the SQL is built, then carried by the cursor. ⚠️ From the
        // application clock, while writes use `NOW()`: a drift misses the pools touched within it.
        let fence = match sort_column {
            PoolSortColumn::LastSeen => Some(resolve_fence(
                cursor.as_ref().and_then(|c| c.as_of),
                Utc::now(),
            )),
            // Immutable sort column: nothing can move across the cursor.
            PoolSortColumn::FirstSeen => None,
        };

        // Before `resolve_query_mode`, so the restarted listing walks forward from the head.
        let cursor = match &fence {
            Some(f) if !f.keeps_cursor => None,
            _ => cursor,
        };

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();

        let as_of = fence.as_ref().map(|f| f.as_of);

        // ⚠️ A first page is not fenced: it could only hide rows stamped ahead of this clock,
        // with `touched_since` at 0. Its outgoing cursors still carry the anchor.
        let page_fence = as_of.filter(|_| had_cursor);

        let (cursor_sort_value, cursor_pool_address) = match active_cursor {
            Some(c) => (Some(c.sort_value), Some(c.pool_address.to_string())),
            None => (None, None),
        };

        let fee_bps = fee_bps.map(fee_bps_to_numeric).transpose()?;

        let mut qb = build(PaginatedPoolsQuery {
            mode,
            sort,
            cursor_sort_value,
            cursor_pool_address,
            search: search.clone(),
            fee_bps: fee_bps.clone(),
            as_of: page_fence,
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

        let touched_since = self
            .touched_since(as_of, had_cursor, search, fee_bps)
            .await?;

        let page = PageBuilder::new(pools, effective_limit, mode, had_cursor).finalize(|p| {
            let sort_value = match sort_column {
                PoolSortColumn::FirstSeen => p.first_seen_at,
                PoolSortColumn::LastSeen => p.last_seen_at,
            };

            Cursor::Pool(PoolCursor {
                sort_column,
                sort_value,
                pool_address: p.pool_address,
                as_of,
            })
        });

        Ok(PoolPage {
            page,
            as_of,
            touched_since,
        })
    }

    async fn list_fee_tiers(&self) -> RepositoryResult<Vec<FeeTier>> {
        // The most common tiers, re-sorted by fee for display. Ties break on fee,
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

#[cfg(test)]
mod tests {
    use super::{MAX_FENCE_AGE, resolve_fence};
    use chrono::{Duration, TimeZone, Utc};

    fn now() -> chrono::DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000, 0).unwrap()
    }

    /// Mutation: re-minting on every page — the fence must be the same instant across the walk.
    #[test]
    fn a_recent_fence_is_honoured() {
        let claimed = now() - Duration::minutes(3);
        let resolved = resolve_fence(Some(claimed), now());
        assert_eq!(resolved.as_of, claimed);
        assert!(resolved.keeps_cursor);
    }

    /// Mutation: refusing a cursor that predates the field, which would break every in-flight URL.
    #[test]
    fn an_absent_claim_is_minted_without_losing_the_cursor() {
        let resolved = resolve_fence(None, now());
        assert_eq!(resolved.as_of, now());
        assert!(resolved.keeps_cursor);
    }

    /// A hand-edited future fence would make `last_seen_at <= as_of` match
    /// everything while `touched_since` reports 0.
    #[test]
    fn a_fence_in_the_future_is_refused() {
        let resolved = resolve_fence(Some(now() + Duration::hours(6)), now());
        assert_eq!(resolved.as_of, now());
        assert!(!resolved.keeps_cursor);
    }

    #[test]
    fn a_stale_fence_is_refused() {
        let resolved = resolve_fence(Some(now() - MAX_FENCE_AGE - Duration::seconds(1)), now());
        assert_eq!(resolved.as_of, now());
        assert!(!resolved.keeps_cursor);
    }

    /// Mutation: `<=` turned into `<` on the age bound.
    #[test]
    fn the_staleness_bound_is_inclusive() {
        let claimed = now() - MAX_FENCE_AGE;
        let resolved = resolve_fence(Some(claimed), now());
        assert_eq!(resolved.as_of, claimed);
        assert!(resolved.keeps_cursor);
    }

    /// A fresh anchor over a stale `sort_value` would serve again a pool
    /// already shown and touched since.
    #[test]
    fn a_refused_fence_never_keeps_its_cursor() {
        for claimed in [
            now() + Duration::seconds(1),
            now() - MAX_FENCE_AGE - Duration::seconds(1),
            now() - Duration::days(30),
        ] {
            let resolved = resolve_fence(Some(claimed), now());
            assert_eq!(resolved.as_of, now(), "refused claim must not anchor");
            assert!(
                !resolved.keeps_cursor,
                "a refused fence must take its cursor with it: {claimed}"
            );
        }
    }
}
