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

#![cfg(feature = "integration-tests")]

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

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

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

    assert_eq!(addrs(&page.items), vec![pk(1), pk(2), pk(3)]);
}

#[sqlx::test]
async fn first_seen_desc_orders_newest_first(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::FirstSeenDesc, 50))
        .await
        .unwrap();

    assert_eq!(addrs(&page.items), vec![pk(3), pk(2), pk(1)]);
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
    assert_eq!(addrs(&page.items), vec![pk(2), pk(3), pk(1)]);
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
    assert_eq!(addrs(&page.items), vec![pk(1), pk(3), pk(2)]);
}

// ── Pagination: walk forward page by page ───────────────────────────

#[sqlx::test]
async fn forward_pagination_covers_all_rows_without_overlap(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);
    let sort = PoolSort::FirstSeenDesc; // expected order: C, B, A

    // Page 1: limit 2 → [C, B], has next.
    let p1 = repo.find_paginated(base_query(sort, 2)).await.unwrap();
    assert_eq!(addrs(&p1.items), vec![pk(3), pk(2)]);
    assert!(p1.is_first);
    assert!(!p1.is_last);
    assert!(p1.next_cursor.is_some());

    // Page 2: from next_cursor → [A], last page.
    let cursor = extract_pool_cursor(p1.next_cursor.as_ref().unwrap());
    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(cursor),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&p2.items), vec![pk(1)]);
    assert!(!p2.is_first);
    assert!(p2.is_last);
    assert!(p2.next_cursor.is_none());
}

// ── Round-trip: Next then Prev returns to the same page ─────────────

#[sqlx::test]
async fn next_then_prev_returns_to_first_page(pool: PgPool) {
    seed_three(&pool).await;
    let repo = PgPoolRepository::new(pool);
    let sort = PoolSort::FirstSeenDesc; // C, B, A

    // Page 1 [C, B], go Next to page 2.
    let p1 = repo.find_paginated(base_query(sort, 2)).await.unwrap();
    let next = extract_pool_cursor(p1.next_cursor.as_ref().unwrap());

    let p2 = repo
        .find_paginated(PoolListQuery {
            cursor: Some(next),
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&p2.items), vec![pk(1)]);

    // From page 2, go Prev — must return to [C, B] in display order.
    let prev = extract_pool_cursor(p2.prev_cursor.as_ref().unwrap());
    let back = repo
        .find_paginated(PoolListQuery {
            cursor: Some(prev),
            direction: PageDirection::Prev,
            ..base_query(sort, 2)
        })
        .await
        .unwrap();
    assert_eq!(addrs(&back.items), vec![pk(3), pk(2)]);
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
    assert_eq!(addrs(&page.items), vec![pk(2), pk(1)]);
    assert!(page.is_last);
    assert!(!page.is_first);
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

    assert_eq!(addrs(&explicit_first.items), addrs(&implicit_first.items));
    assert!(explicit_first.is_first);
}

// ── Empty table ─────────────────────────────────────────────────────

#[sqlx::test]
async fn empty_table_yields_empty_page_at_both_boundaries(pool: PgPool) {
    let repo = PgPoolRepository::new(pool);

    let page = repo
        .find_paginated(base_query(PoolSort::FirstSeenDesc, 50))
        .await
        .unwrap();

    assert!(page.items.is_empty());
    assert!(page.is_first);
    assert!(page.is_last);
    assert!(page.next_cursor.is_none());
    assert!(page.prev_cursor.is_none());
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
    assert_eq!(addrs(&page.items), vec![pk(1), pk(2)]);
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

    assert!(page.items.is_empty());
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
