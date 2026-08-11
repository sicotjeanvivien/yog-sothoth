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
    /// traversal's fence — see `PoolPage::touched_since`.
    ///
    /// Skipped, and reported as `0`, in the two cases where it is knowably
    /// nothing: an unfenced sort, and the first page of a traversal (the fence
    /// was minted an instant ago, so nothing can be above it — running the
    /// query there would be a round-trip to be told zero).
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
///
/// A listing is read in minutes; an hour is already far past any live
/// traversal. Beyond it the cursor is not a reader mid-scroll, it is a
/// bookmarked or shared URL, and replaying its anchor serves a snapshot of the
/// past while `touched_since` counts most of the table. Re-anchoring costs that
/// one hop its guarantee — exactly what a cursor minted before the fence
/// existed already costs — and buys back a page that means something.
const MAX_FENCE_AGE: chrono::Duration = chrono::Duration::hours(1);

/// The anchor a traversal runs under, once the cursor's claim has been vetted.
struct ResolvedFence {
    as_of: DateTime<Utc>,
    /// Whether the cursor that made the claim may still anchor the keyset.
    ///
    /// `false` means the claim was refused, and then the cursor goes with it:
    /// a fresh anchor over a stale `sort_value` is not a repair, it is the
    /// original bug for one hop — under `LastSeenAsc` a pool already shown and
    /// touched since satisfies both `> sort_value` and `<= now`, and is served
    /// a second time. Restarting at the head of the listing is the only answer
    /// that stays consistent, and `Page::is_first` tells the client it happened.
    keeps_cursor: bool,
}

/// Resolve the fence a traversal runs under, from the one the cursor claims.
///
/// A cursor is base64 JSON handed back by the client: unsigned, editable, and
/// replayable at any later date. Every other field it carries is validated, and
/// this one bounds the query, so it is validated too:
///
/// - **above `now`** — an `as_of` in the future makes `last_seen_at <= as_of`
///   match everything. The traversal is silently unanchored, which is the
///   original bug back in full, and `touched_since` reports `0`, so the client
///   states that nothing moved. Refusing it stops a hand-edited cursor from
///   turning the fix off while claiming it is on.
/// - **older than [`MAX_FENCE_AGE`]** — no longer a reader mid-scroll, and its
///   ancient `as_of` also drains the selectivity of the `touched_since` range
///   predicate, degrading it toward a full scan on caller-controlled input.
///
/// A cursor claiming **nothing** is the one case that keeps its place: that is
/// a cursor minted before this field existed, held by a reader who was walking
/// the listing when the change shipped. Rejecting those would break every
/// in-flight URL at deploy time to fix one hop of one traversal.
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
        // Claimed and refused — the cursor is refused with it.
        Some(_) => ResolvedFence {
            as_of: now,
            keeps_cursor: false,
        },
    }
}

/// How many fee tiers the filter offers. The dozen-or-so real tiers hold the
/// vast majority of pools; capping at the most common keeps the dropdown short
/// and drops the long tail of one-off dynamic-fee/launch values.
const FEE_TIER_LIMIT: i64 = 8;

/// Convert a domain `fee_bps` (`rust_decimal::Decimal`) to the `BigDecimal`
/// that NUMERIC binds to at the persistence boundary. Round-trips through the
/// exact decimal string — never lossy for the small fee values we store.
pub(super) fn fee_bps_to_numeric(
    fee_bps: rust_decimal::Decimal,
) -> RepositoryResult<sqlx::types::BigDecimal> {
    sqlx::types::BigDecimal::from_str(&fee_bps.to_string())
        .map_err(|e| RepositoryError::Integrity(format!("invalid fee_bps decimal: {e}")))
}

#[async_trait]
impl PoolRepository for PgPoolRepository {
    /// # The rule that makes `last_seen_at` usable as a sort key
    ///
    /// **Clamp both candidates to the database clock, then keep the later.**
    /// Two halves, and dropping either one breaks a different thing.
    ///
    /// *Keep the later* is what the pool listing's snapshot fence (`PoolPage`)
    /// rests on: the column only ever growing is what turns "a touched row
    /// moves across the cursor" into "a touched row leaves the result set". A
    /// plain `SET last_seen_at = EXCLUDED.last_seen_at` did not guarantee it —
    /// the value here is the *indexer process* clock captured before the round
    /// trip, [`Self::touch_last_seen`] writes Postgres' `NOW()`, and events are
    /// persisted concurrently, so a touch committing after an older upsert
    /// walked the column backwards. A row moving *down* re-enters a traversal
    /// below a cursor already passed and is served twice; the fence bounds from
    /// above only and cannot catch that.
    ///
    /// *Clamp to the database clock* is what keeps the first half from becoming
    /// a trap. `GREATEST` alone makes a value that is **ahead** of real time
    /// permanent — one NTP correction or a suspended VM on the indexer host and
    /// the row is stuck in the future, where nothing can lower it again. That
    /// pool then fails `last_seen_at <= as_of` on every page of every listing
    /// while satisfying `last_seen_at > as_of` in the `touched_since` count:
    /// invisible forever, and reported forever as having just moved. The
    /// clamp also *repairs* such a row on its next write rather than only
    /// declining to create a new one.
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

    /// Same rule as [`Self::upsert`] — clamp both candidates to the database
    /// clock, keep the later — collapsed by the fact that the incoming value
    /// *is* that clock. `GREATEST(LEAST(last_seen_at, NOW()), NOW())` is
    /// `NOW()` for every input, so that is what is written.
    ///
    /// Which is what the plain `NOW()` here always was; the bug was never on
    /// this side. Spelled out because the bare statement now reads like the
    /// guard is missing at one write site out of two — it is not, it is the
    /// same guard with nothing left to compare.
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

        // The snapshot fence is minted HERE, not by the caller: this is the
        // only place that builds this SQL, so the invariant cannot be
        // forgotten by a future caller. It is carried forward by the cursor,
        // and re-minted whenever there is none.
        //
        // Taken from the application clock, while the write path bounds
        // `last_seen_at` by Postgres' `NOW()`. Same host today, so the two
        // agree. A drift either way is bounded by NTP and costs at most the
        // pools touched inside that window; reading `NOW()` from the database
        // would close it at the price of a round-trip on every listing, which
        // is not a trade worth making here — but it is the trade, written down
        // rather than re-derived.
        let fence = match sort_column {
            PoolSortColumn::LastSeen => Some(resolve_fence(
                cursor.as_ref().and_then(|c| c.as_of),
                Utc::now(),
            )),
            // Immutable sort column: nothing can move across the cursor.
            PoolSortColumn::FirstSeen => None,
        };

        // A refused fence takes its cursor with it — see `ResolvedFence`. Done
        // before `resolve_query_mode` so the restarted listing is a forward
        // walk from the head, not a backward one from nowhere.
        let cursor = match &fence {
            Some(f) if !f.keeps_cursor => None,
            _ => cursor,
        };

        let mode = resolve_query_mode(position, &cursor, direction);

        let active_cursor = if position.is_some() { None } else { cursor };
        let had_cursor = active_cursor.is_some();

        let as_of = fence.as_ref().map(|f| f.as_of);

        // The fence bounds the PAGE only once there is a cursor to protect.
        // Without one there is no keyset assumption in play: an unanchored
        // first page just wants the head of the list, and bounding it could
        // only *hide* rows — those whose stored instant runs ahead of this
        // process's clock — on the most-visited page of the product, with
        // `touched_since` short-circuited to 0 so nothing would report the
        // loss. The anchor is still minted and stamped into the outgoing
        // cursors, so every page after this one is fenced.
        let page_fence = as_of.filter(|_| had_cursor);

        let (cursor_sort_value, cursor_pool_address) = match active_cursor {
            Some(c) => (Some(c.sort_value), Some(c.pool_address.to_string())),
            None => (None, None),
        };

        // NUMERIC binds to BigDecimal at the persistence boundary — same
        // lossless string round-trip as the write path.
        let fee_bps = fee_bps.map(fee_bps_to_numeric).transpose()?;

        // Build the dynamic query (ORDER BY + keyset + fence + search + fee)
        // and run it. Mapping goes through PoolRow (FromRow) then
        // Pool::try_from.
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

#[cfg(test)]
mod tests {
    use super::{MAX_FENCE_AGE, resolve_fence};
    use chrono::{Duration, TimeZone, Utc};

    fn now() -> chrono::DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000, 0).unwrap()
    }

    /// A live traversal keeps its anchor, and its cursor. Re-minting on every
    /// page would undo the whole point: the fence has to be the *same* instant
    /// across the walk.
    #[test]
    fn a_recent_fence_is_honoured() {
        let claimed = now() - Duration::minutes(3);
        let resolved = resolve_fence(Some(claimed), now());
        assert_eq!(resolved.as_of, claimed);
        assert!(resolved.keeps_cursor);
    }

    /// A cursor claiming nothing was minted before the field existed, and is
    /// held by a reader who was mid-listing when this shipped. It gets a fresh
    /// anchor and **keeps its place**: refusing it would break every in-flight
    /// URL at deploy time to fix one hop of one traversal.
    #[test]
    fn an_absent_claim_is_minted_without_losing_the_cursor() {
        let resolved = resolve_fence(None, now());
        assert_eq!(resolved.as_of, now());
        assert!(resolved.keeps_cursor);
    }

    /// A cursor is unsigned base64 the client hands back, so a fence in the
    /// future is reachable by hand-editing one. Honouring it would make
    /// `last_seen_at <= as_of` match everything: the traversal unanchored, the
    /// original skip/duplicate bug back, and `touched_since` reporting 0 —
    /// the fix switched off while the response claims it is on.
    #[test]
    fn a_fence_in_the_future_is_refused() {
        let resolved = resolve_fence(Some(now() + Duration::hours(6)), now());
        assert_eq!(resolved.as_of, now());
        assert!(!resolved.keeps_cursor);
    }

    /// Past the cutoff the cursor is a bookmark, not a reader mid-scroll:
    /// replaying it serves a stale snapshot and leaves the `touched_since`
    /// range predicate selecting most of the table.
    #[test]
    fn a_stale_fence_is_refused() {
        let resolved = resolve_fence(Some(now() - MAX_FENCE_AGE - Duration::seconds(1)), now());
        assert_eq!(resolved.as_of, now());
        assert!(!resolved.keeps_cursor);
    }

    /// Exactly at the cutoff is still honoured — the bound is inclusive, and
    /// saying so pins which side of `<=` the code is on.
    #[test]
    fn the_staleness_bound_is_inclusive() {
        let claimed = now() - MAX_FENCE_AGE;
        let resolved = resolve_fence(Some(claimed), now());
        assert_eq!(resolved.as_of, claimed);
        assert!(resolved.keeps_cursor);
    }

    /// The pairing that matters, stated on its own: a refused claim never
    /// leaves a fresh anchor sitting over a stale `sort_value`. That
    /// combination is the original bug for one hop — under `LastSeenAsc` a
    /// pool already shown and touched since would satisfy both halves of the
    /// predicate and be served a second time.
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
