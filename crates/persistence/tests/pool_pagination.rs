//! Integration tests for PgPoolRepository::find_paginated.
//!
//! Gated behind the `integration-tests` feature: they require a live
//! Postgres (provided by sqlx::test, which creates an isolated
//! database per test and applies the migrations). The CI job
//! `test-integration` runs them; a plain `cargo test` skips them.
//!
//! These cover what the Couche-1 unit tests cannot: that the
//! assembled SQL actually runs, orders rows correctly, and that
//! Next/Prev/First/Last navigation is internally consistent against
//! a real dataset.

use super::helpers::pk;
use chrono::{DateTime, TimeZone, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use std::str::FromStr;

use yog_core::{
    PageDirection, PagePosition, PoolSort,
    domain::{Pool, PoolCatalog, PoolCursor, PoolListQuery, Protocol},
};
use yog_persistence::PgPoolRepository;

// ── Seed helpers ────────────────────────────────────────────────────

fn ts(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).unwrap()
}

/// Insert a pool with explicit first_seen / last_seen. We bypass the
/// repository's `upsert` (which sets both timestamps equal) because
/// these tests need first_seen and last_seen to differ, to tell the
/// two sort columns apart.
async fn seed_pool(
    pool: &PgPool,
    addr: Pubkey,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
) {
    sqlx::query(
        r#"
        INSERT INTO pools
            (pool_address, protocol, token_a_mint, token_b_mint,
             first_seen_at, last_seen_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(addr.to_string())
    .bind(Protocol::MeteoraDammV2.as_str()) // adapt variant name
    .bind(pk(200).to_string())
    .bind(pk(201).to_string())
    .bind(first_seen)
    .bind(last_seen)
    .execute(pool)
    .await
    .expect("seed insert failed");
}

/// Seed three pools with distinct, interleaved first/last_seen so the
/// two sort columns produce DIFFERENT orderings — this is what makes
/// the tests able to catch "sorted by the wrong column".
///
///   addr | first_seen | last_seen
///   A(1) |   100      |   300
///   B(2) |   200      |   100
///   C(3) |   300      |   200
///
/// first_seen ASC  → A, B, C
/// first_seen DESC → C, B, A
/// last_seen  ASC  → B, C, A
/// last_seen  DESC → A, C, B
async fn seed_three(pool: &PgPool) {
    seed_pool(pool, pk(1), ts(100), ts(300)).await;
    seed_pool(pool, pk(2), ts(200), ts(100)).await;
    seed_pool(pool, pk(3), ts(300), ts(200)).await;
}

fn addrs(pools: &[Pool]) -> Vec<Pubkey> {
    pools.iter().map(|p| p.pool_address).collect()
}

/// A `PoolListQuery` with the given sort and page size and every other
/// dimension at its neutral default (no cursor, forward, no position, no
/// filter). Tests that need a cursor/position/filter override the field
/// via struct-update syntax: `PoolListQuery { cursor: Some(c), ..base_query(sort, 2) }`.
fn base_query(sort: PoolSort, limit: i64) -> PoolListQuery {
    PoolListQuery {
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        sort,
        search: None,
        fee_bps: None,
        limit,
    }
}

// ── Ordering: the four sorts produce the documented order ───────────

#[sqlx::test]
async fn first_seen_asc_orders_oldest_first(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::FirstSeenAsc, 50))
        .await
        .unwrap();

    assert_eq!(addrs(&page.page.items), vec![pk(1), pk(2), pk(3)]);
}

#[sqlx::test]
async fn first_seen_desc_orders_newest_first(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::FirstSeenDesc, 50))
        .await
        .unwrap();

    assert_eq!(addrs(&page.page.items), vec![pk(3), pk(2), pk(1)]);
}

#[sqlx::test]
async fn last_seen_asc_orders_by_last_seen(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::LastSeenAsc, 50))
        .await
        .unwrap();

    // last_seen ASC → B(100), C(200), A(300)
    assert_eq!(addrs(&page.page.items), vec![pk(2), pk(3), pk(1)]);
}

#[sqlx::test]
async fn last_seen_desc_orders_by_last_seen(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::LastSeenDesc, 50))
        .await
        .unwrap();

    // last_seen DESC → A(300), C(200), B(100)
    assert_eq!(addrs(&page.page.items), vec![pk(1), pk(3), pk(2)]);
}

// ── Pagination: walk forward page by page ───────────────────────────

#[sqlx::test]
async fn forward_pagination_covers_all_rows_without_overlap(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);
    let sort = PoolSort::FirstSeenDesc; // expected order: C, B, A

    // Page 1: limit 2 → [C, B], has next.
    let p1 = repo.find_paginated(base_query(sort, 2)).await.unwrap();
    assert_eq!(addrs(&p1.page.items), vec![pk(3), pk(2)]);
    assert!(p1.page.is_first);
    assert!(!p1.page.is_last);
    assert!(p1.page.next_cursor.is_some());

    // Page 2: from next_cursor → [A], last page.
    let cursor = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&p2.page.items), vec![pk(1)]);
    assert!(!p2.page.is_first);
    assert!(p2.page.is_last);
    assert!(p2.page.next_cursor.is_none());
}

// ── Round-trip: Next then Prev returns to the same page ─────────────

#[sqlx::test]
async fn next_then_prev_returns_to_first_page(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);
    let sort = PoolSort::FirstSeenDesc; // C, B, A

    // Page 1 [C, B], go Next to page 2.
    let p1 = repo.find_paginated(base_query(sort, 2)).await.unwrap();
    let next = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());

    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(next),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&p2.page.items), vec![pk(1)]);

    // From page 2, go Prev — must return to [C, B] in display order.
    let prev = extract_pool_cursor(p2.page.prev_cursor.as_ref().unwrap());
    let back = repo
        .find_paginated(PoolListQuery {
            cursor: Some(prev),
            direction: PageDirection::Prev,
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&back.page.items), vec![pk(3), pk(2)]);
}

// ── Position jumps: First / Last ────────────────────────────────────

#[sqlx::test]
async fn position_last_jumps_to_end(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);
    let sort = PoolSort::FirstSeenDesc; // C, B, A

    // Last page with limit 2 → the oldest slice [A], in display order.
    let page = repo
        .find_paginated(PoolListQuery {
            position: Some(PagePosition::Last),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();

    // The very last item in C,B,A order is A; a 2-wide last page is
    // [B, A] in display order.
    assert_eq!(addrs(&page.page.items), vec![pk(2), pk(1)]);
    assert!(page.page.is_last);
    assert!(!page.page.is_first);
}

#[sqlx::test]
async fn position_first_matches_unanchored_first_page(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);
    let sort = PoolSort::LastSeenDesc;

    let explicit_first = repo
        .find_paginated(PoolListQuery {
            position: Some(PagePosition::First),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    let implicit_first = repo.find_paginated(base_query(sort, 2)).await.unwrap();

    assert_eq!(
        addrs(&explicit_first.page.items),
        addrs(&implicit_first.page.items)
    );
    assert!(explicit_first.page.is_first);
}

// ── Empty table ─────────────────────────────────────────────────────

#[sqlx::test]
async fn empty_table_yields_empty_page_at_both_boundaries(pool: PgPool) {
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::FirstSeenDesc, 50))
        .await
        .unwrap();

    assert!(page.page.items.is_empty());
    assert!(page.page.is_first);
    assert!(page.page.is_last);
    assert!(page.page.next_cursor.is_none());
    assert!(page.page.prev_cursor.is_none());
}

// ── Fee-tier filter + option list ───────────────────────────────────

/// Set a pool's base fee, so the fee-filter tests can seed distinct tiers
/// on top of the timestamp-only `seed_pool`.
async fn set_fee(pool: &PgPool, addr: Pubkey, fee_bps: rust_decimal::Decimal) {
    sqlx::query("UPDATE pools SET fee_bps = $2 WHERE pool_address = $1")
        .bind(addr.to_string())
        .bind(sqlx::types::BigDecimal::from_str(&fee_bps.to_string()).unwrap())
        .execute(pool)
        .await
        .expect("set fee failed");
}

#[sqlx::test]
async fn fee_bps_filter_returns_only_matching_tier(pool: PgPool) {
    seed_three(&pool).await;
    // A, B on the 25 bps tier; C on 100 bps.
    set_fee(&pool, pk(1), dec(25)).await;
    set_fee(&pool, pk(2), dec(25)).await;
    set_fee(&pool, pk(3), dec(100)).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(PoolListQuery {
            fee_bps: Some(dec(25)),
            ..base_query(PoolSort::FirstSeenAsc, 50)
        })
        .await
        .unwrap();

    // Only the 25 bps pools, in first_seen ASC order: A, B (C excluded).
    assert_eq!(addrs(&page.page.items), vec![pk(1), pk(2)]);
}

#[sqlx::test]
async fn fee_bps_filter_no_match_yields_empty_page(pool: PgPool) {
    seed_three(&pool).await;
    set_fee(&pool, pk(1), dec(25)).await;
    let repo = PgPoolRepository::new(pool);

    // A tier no pool carries → an empty page, not an error.
    let page = repo
        .find_paginated(PoolListQuery {
            fee_bps: Some(dec(9999)),
            ..base_query(PoolSort::FirstSeenAsc, 50)
        })
        .await
        .unwrap();

    assert!(page.page.items.is_empty());
}

#[sqlx::test]
async fn list_fee_tiers_returns_distinct_tiers_with_counts_ascending(pool: PgPool) {
    seed_three(&pool).await;
    // Two pools share 25 bps; one is 100 bps; NULL-fee pools must not surface.
    set_fee(&pool, pk(1), dec(100)).await;
    set_fee(&pool, pk(2), dec(25)).await;
    set_fee(&pool, pk(3), dec(25)).await;
    let repo = PgPoolRepository::new(pool);

    let tiers = repo.list_fee_tiers().await.unwrap();

    // Distinct, each with its count, ascending by fee for display (25 not
    // duplicated, NULL excluded).
    assert_eq!(tiers, vec![fee_tier(dec(25), 2), fee_tier(dec(100), 1)]);
}

#[sqlx::test]
async fn list_fee_tiers_keeps_only_the_most_common_capped(pool: PgPool) {
    // Nine distinct tiers, one pool each (all count 1). The cap keeps the top
    // 8; the count tie breaks by fee ASC, so the highest fee (90) is the one
    // dropped. The survivors come back ascending for display.
    for i in 1..=9u8 {
        let addr = pk(50 + i);
        seed_pool(&pool, addr, ts(i as i64 * 10), ts(i as i64 * 10)).await;
        set_fee(&pool, addr, dec(i as i64 * 10)).await;
    }
    let repo = PgPoolRepository::new(pool);

    let tiers = repo.list_fee_tiers().await.unwrap();

    let fees: Vec<rust_decimal::Decimal> = tiers.iter().map(|t| t.fee_bps).collect();
    assert_eq!(fees, (1..=8i64).map(|i| dec(i * 10)).collect::<Vec<_>>());
    assert!(!fees.contains(&dec(90)));
}

#[sqlx::test]
async fn list_fee_tiers_empty_when_no_fees_resolved(pool: PgPool) {
    seed_three(&pool).await; // all three left with NULL fee_bps
    let repo = PgPoolRepository::new(pool);

    let tiers = repo.list_fee_tiers().await.unwrap();

    assert!(tiers.is_empty());
}

/// Small `Decimal` literal helper for the fee tiers.
fn dec(n: i64) -> rust_decimal::Decimal {
    rust_decimal::Decimal::from(n)
}

/// Build an expected `FeeTier` for assertions.
fn fee_tier(fee_bps: rust_decimal::Decimal, pool_count: i64) -> yog_core::domain::FeeTier {
    yog_core::domain::FeeTier {
        fee_bps,
        pool_count,
    }
}

// ── Helper: pull a PoolCursor out of the Cursor enum ────────────────

fn extract_pool_cursor(cursor: &yog_core::Cursor) -> PoolCursor {
    match cursor {
        yog_core::Cursor::Pool(c) => c.clone(),
        other => panic!("expected a Pool cursor, got {other:?}"),
    }
}

// ── Snapshot fence: paginating over a column that moves ─────────────
//
// `last_seen_at` is rewritten on every event touching the pool, and a
// keyset cursor assumes its sort key holds still. The fence pins the
// traversal to the instant it started so a touched row leaves the result
// set instead of moving across the cursor — see
// `yog_core::domain::PoolPage`.
//
// Every test below mutates a pool *between two pages*, which is the
// situation the production bug needed and no earlier test created.
//
// ⚠️ Asserting "the touched pool is absent" would NOT catch the bug:
// absent is exactly what the broken code produced. What separates the two
// is that the pool never reappears at a *different rank*, that the
// duplicate is gone, and that the departure is counted.

/// Push a pool's `last_seen_at` above any fence a running test can mint.
/// An explicit hour ahead, not `NOW()`: the assertion must not depend on
/// how many microseconds elapsed since the fence was taken.
async fn touch_pool(pool: &PgPool, addr: Pubkey) {
    sqlx::query("UPDATE pools SET last_seen_at = $2 WHERE pool_address = $1")
        .bind(addr.to_string())
        .bind(Utc::now() + chrono::Duration::hours(1))
        .execute(pool)
        .await
        .expect("touch failed");
}

/// Ascending traversal, the duplicate case: a pool already shown moves to
/// the tail and is served a second time. Under the fence it leaves instead.
#[sqlx::test]
async fn last_seen_asc_does_not_serve_a_touched_pool_twice(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool.clone());
    let sort = PoolSort::LastSeenAsc; // B(100), C(200), A(300)

    // Page 1 → [B].
    let p1 = repo.find_paginated(base_query(sort, 1)).await.unwrap();
    assert_eq!(addrs(&p1.page.items), vec![pk(2)]);

    // B is touched: its last_seen jumps past every remaining row, which
    // without a fence would place it after the cursor — again.
    touch_pool(&pool, pk(2)).await;

    let cursor = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..base_query(sort, 50)
        })
        .await
        .unwrap();

    let seen: Vec<Pubkey> = addrs(&p1.page.items)
        .into_iter()
        .chain(addrs(&p2.page.items))
        .collect();
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        seen.len(),
        unique.len(),
        "a pool was served twice across the traversal: {seen:?}"
    );
}

/// Descending traversal, backward navigation: a pool touched mid-traversal
/// has moved to the head of the live list. It must not come back into a
/// page of *this* traversal, where it would appear at a rank the reader
/// never saw it in.
#[sqlx::test]
async fn last_seen_desc_backward_page_excludes_a_touched_pool(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool.clone());
    let sort = PoolSort::LastSeenDesc; // A(300), C(200), B(100)

    // Page 1 → [A, C], then page 2 → [B].
    let p1 = repo.find_paginated(base_query(sort, 2)).await.unwrap();
    assert_eq!(addrs(&p1.page.items), vec![pk(1), pk(3)]);
    let next = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(next),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&p2.page.items), vec![pk(2)]);

    // C, already read on page 1, becomes the most recently active pool.
    touch_pool(&pool, pk(3)).await;

    let prev = extract_pool_cursor(p2.page.prev_cursor.as_ref().unwrap());
    let back = repo
        .find_paginated(PoolListQuery {
            cursor: Some(prev),
            direction: PageDirection::Prev,
            ..base_query(sort, 2)
        })
        .await
        .unwrap();

    assert!(
        !addrs(&back.page.items).contains(&pk(3)),
        "a pool touched mid-traversal re-entered it: {:?}",
        addrs(&back.page.items)
    );
}

/// The departures are counted. This is what keeps a descending listing
/// honest: it cannot show the pools that moved above its fence, so it says
/// how many did.
#[sqlx::test]
async fn touched_since_counts_the_pools_that_left(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool.clone());
    let sort = PoolSort::LastSeenDesc;

    let p1 = repo.find_paginated(base_query(sort, 1)).await.unwrap();
    assert!(p1.as_of.is_some(), "a last_seen traversal must be anchored");
    assert_eq!(
        p1.touched_since, 0,
        "the fence was just minted; nothing can be above it"
    );

    touch_pool(&pool, pk(2)).await;
    touch_pool(&pool, pk(3)).await;

    let cursor = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..base_query(sort, 50)
        })
        .await
        .unwrap();

    assert_eq!(
        p2.as_of, p1.as_of,
        "the fence must be carried, not re-minted"
    );
    assert_eq!(p2.touched_since, 2);

    // Both remaining pools left, so the page is legitimately EMPTY — with no
    // cursor either side, since there is no row to build one from. That is
    // correct here and a dead end for whoever renders it: the count is the
    // only thing left on the page that can explain it and offer a way out.
    // Asserting the shape is what makes that obligation visible from the
    // repository, rather than something a UI discovers in production.
    assert!(
        p2.page.items.is_empty(),
        "expected every remaining pool to have left: {:?}",
        addrs(&p2.page.items)
    );
    assert!(p2.page.next_cursor.is_none() && p2.page.prev_cursor.is_none());
}

/// The complement of the test above, and the reason both exist: an empty page
/// is the *right* answer only when everything left. A traversal with rows
/// remaining must still serve them.
///
/// This one does not guard the fence's presence (dropping the fence leaves it
/// green — the moved pool is out of the keyset window either way); it guards
/// the fence's **direction and extent**. Inverting the comparison, which is
/// the realistic typo, reddens it.
#[sqlx::test]
async fn fence_removes_only_the_pools_that_moved(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool.clone());
    let sort = PoolSort::LastSeenDesc; // A(300), C(200), B(100)

    let p1 = repo.find_paginated(base_query(sort, 1)).await.unwrap();
    assert_eq!(addrs(&p1.page.items), vec![pk(1)]);

    // Only C moves. B is untouched and must still be served.
    touch_pool(&pool, pk(3)).await;

    let cursor = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..base_query(sort, 50)
        })
        .await
        .unwrap();

    assert_eq!(
        addrs(&p2.page.items),
        vec![pk(2)],
        "the fence must drop the moved pool and nothing else"
    );
    assert_eq!(p2.touched_since, 1);
}

/// The count is scoped to the same population as the page. A pool the
/// reader filtered out is not a pool that left *their* listing.
#[sqlx::test]
async fn touched_since_ignores_pools_outside_the_active_filter(pool: PgPool) {
    seed_three(&pool).await;
    set_fee(&pool, pk(1), dec(25)).await;
    set_fee(&pool, pk(2), dec(25)).await;
    set_fee(&pool, pk(3), dec(100)).await; // outside the filter below
    let repo = PgPoolRepository::new(pool.clone());

    let query = || PoolListQuery {
        fee_bps: Some(dec(25)),
        ..base_query(PoolSort::LastSeenDesc, 1)
    };

    let p1 = repo.find_paginated(query()).await.unwrap();
    assert_eq!(addrs(&p1.page.items), vec![pk(1)]);

    // One pool inside the filter, one outside. Only the first has left
    // anything the reader can see.
    touch_pool(&pool, pk(2)).await;
    touch_pool(&pool, pk(3)).await;

    let cursor = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..query()
        })
        .await
        .unwrap();

    // B (25 bps) left the filtered listing; C (100 bps) was never in it.
    assert!(
        p2.page.items.is_empty(),
        "expected the filtered remainder to be empty: {:?}",
        addrs(&p2.page.items)
    );
    assert_eq!(
        p2.touched_since, 1,
        "the count must apply the page's own filters"
    );
}

/// The immutable sort column gets no fence — and must not: fencing it
/// would hide pools for a mutation that cannot affect their order.
#[sqlx::test]
async fn first_seen_traversal_is_unfenced(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool.clone());
    let sort = PoolSort::FirstSeenDesc; // C, B, A by first_seen

    let p1 = repo.find_paginated(base_query(sort, 2)).await.unwrap();
    assert_eq!(addrs(&p1.page.items), vec![pk(3), pk(2)]);
    assert!(p1.as_of.is_none(), "an immutable sort needs no fence");
    assert_eq!(p1.touched_since, 0);

    // A's activity changes; its first_seen, and so its rank, does not.
    touch_pool(&pool, pk(1)).await;

    let cursor = extract_pool_cursor(p1.page.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();

    assert_eq!(
        addrs(&p2.page.items),
        vec![pk(1)],
        "a touched pool must still be reachable under an immutable sort"
    );
}

// ── The invariant the fence rests on: `last_seen_at` only grows ─────
//
// The fence turns "a touched row moves across the cursor" into "a touched
// row leaves the result set" *because* the column only ever increases. It
// bounds from above only, so a row moving DOWN is invisible to it — it
// re-enters the traversal below a cursor already passed and gets served a
// second time. Nothing enforced that until `GREATEST` did: `upsert` writes
// the indexer process clock, `touch_last_seen` writes Postgres' `NOW()`,
// and events are persisted concurrently.

/// A stale observation must not walk the column backwards, whichever writer
/// carries it.
#[sqlx::test]
async fn last_seen_at_never_moves_backwards(pool: PgPool) {
    use yog_core::domain::PoolRepository;

    seed_pool(&pool, pk(1), ts(100), ts(300)).await;
    let repo = PgPoolRepository::new(pool.clone());

    // An upsert carrying an *older* instant than what is stored — a swap
    // whose event was decoded before a concurrent touch committed.
    repo.upsert(&Pool {
        pool_address: pk(1),
        protocol: Protocol::MeteoraDammV2,
        token_a_mint: Some(pk(200)),
        token_b_mint: Some(pk(201)),
        fee_bps: None,
        first_seen_at: ts(100),
        last_seen_at: ts(200),
    })
    .await
    .unwrap();

    assert_eq!(
        last_seen_of(&pool, pk(1)).await,
        ts(300),
        "an older observation must not lower last_seen_at"
    );

    // A newer one still moves it forward — the guard must not freeze the
    // column, only floor it.
    repo.upsert(&Pool {
        pool_address: pk(1),
        protocol: Protocol::MeteoraDammV2,
        token_a_mint: Some(pk(200)),
        token_b_mint: Some(pk(201)),
        fee_bps: None,
        first_seen_at: ts(100),
        last_seen_at: ts(400),
    })
    .await
    .unwrap();

    assert_eq!(last_seen_of(&pool, pk(1)).await, ts(400));

    // `touch_last_seen` writes NOW(), which is later than every seeded
    // instant, so it must move the column forward and not be floored away.
    repo.touch_last_seen(&pk(1)).await.unwrap();
    assert!(
        last_seen_of(&pool, pk(1)).await > ts(400),
        "a touch at NOW() must still advance the column"
    );
}

/// Read one pool's `last_seen_at` straight from the table.
async fn last_seen_of(pool: &PgPool, addr: Pubkey) -> DateTime<Utc> {
    sqlx::query_scalar::<_, DateTime<Utc>>("SELECT last_seen_at FROM pools WHERE pool_address = $1")
        .bind(addr.to_string())
        .fetch_one(pool)
        .await
        .expect("read last_seen_at failed")
}
