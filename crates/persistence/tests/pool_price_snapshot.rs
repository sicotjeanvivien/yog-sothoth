//! The pool price snapshot view, through `PgPoolPriceSnapshotRepository`: a
//! pool with every input comes back whole, one missing an input not at all.

use super::helpers::{pk, price_at};
use chrono::{DateTime, Duration, SubsecRound, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::{PoolPriceSnapshotRepository, Protocol};
use yog_persistence::PgPoolPriceSnapshotRepository;

async fn insert_pool(pool: &PgPool, pool_addr: &str, mint_a: &str, mint_b: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(pool_addr)
    .bind(mint_a)
    .bind(mint_b)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_metadata(pool: &PgPool, mint: &str, decimals: i16, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1,$2,$3,$3)",
    )
    .bind(mint)
    .bind(decimals)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_current_state(
    pool: &PgPool,
    pool_addr: &str,
    sqrt_price: Option<&str>,
    swap_at: Option<DateTime<Utc>>,
    (reserve_a, reserve_b): (i64, i64),
) {
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_sqrt_price, last_swap_at, last_slot, last_event_index)
         VALUES ($1,'meteora_damm_v2',NOW(),'swap','sig',$4,$5,$2::NUMERIC,$3,1,0)",
    )
    .bind(pool_addr)
    .bind(sqrt_price)
    .bind(swap_at)
    .bind(reserve_a)
    .bind(reserve_b)
    .execute(pool)
    .await
    .unwrap();
}

/// Pool `pk(1)` on mints `pk(2)` (6 decimals) and `pk(3)` (9), last swapped at
/// `swap_at` with sqrt_price = 2^64 (raw price 1.0 before decimal rescaling).
async fn seed_swapped_pool(pool: &PgPool, swap_at: DateTime<Utc>) -> (String, String) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(pool, &pool_addr, &mint_a, &mint_b).await;
    insert_metadata(pool, &mint_a, 6, swap_at).await;
    insert_metadata(pool, &mint_b, 9, swap_at).await;
    insert_current_state(
        pool,
        &pool_addr,
        Some("18446744073709551616"),
        Some(swap_at),
        (0, 0),
    )
    .await;
    (mint_a, mint_b)
}

#[sqlx::test]
async fn snapshot_joins_state_decimals_and_latest_prices(pool: PgPool) {
    // Truncated to TIMESTAMPTZ's microseconds, so the round trip compares exactly.
    let now = Utc::now().trunc_subsecs(6);
    let swap_at = now - Duration::minutes(5);

    let (mint_a, mint_b) = seed_swapped_pool(&pool, swap_at).await;
    price_at(&pool, &mint_a, "1.5", now - Duration::hours(2)).await;
    price_at(&pool, &mint_a, "2.0", now - Duration::minutes(3)).await;
    price_at(&pool, &mint_b, "80.0", now - Duration::hours(2)).await;
    price_at(&pool, &mint_b, "100.0", now - Duration::minutes(2)).await;

    let repo = PgPoolPriceSnapshotRepository::new(pool.clone());
    let snapshots = repo.latest(Decimal::ZERO).await.unwrap();

    assert_eq!(snapshots.len(), 1);
    let snap = &snapshots[0];
    assert_eq!(snap.pool_address, pk(1));
    assert_eq!(snap.protocol, Protocol::MeteoraDammV2);
    assert_eq!(snap.sqrt_price, 1u128 << 64);
    assert_eq!(snap.last_swap_at, swap_at);
    assert_eq!(snap.decimals_a, 6);
    assert_eq!(snap.decimals_b, 9);
    assert_eq!(snap.price_a_usd, Decimal::from(2));
    assert_eq!(snap.price_b_usd, Decimal::from(100));
    assert_eq!(snap.price_a_fetched_at, now - Duration::minutes(3));
    assert_eq!(snap.price_b_fetched_at, now - Duration::minutes(2));
}

/// Against the view bounding the price's age: it is exempt, the detector gates.
#[sqlx::test]
async fn a_stale_price_is_still_published(pool: PgPool) {
    let now = Utc::now().trunc_subsecs(6);
    let fetched_at = now - Duration::hours(2);

    let (mint_a, mint_b) = seed_swapped_pool(&pool, now).await;
    price_at(&pool, &mint_a, "1.5", fetched_at).await;
    price_at(&pool, &mint_b, "80.0", fetched_at).await;

    let snapshots = PgPoolPriceSnapshotRepository::new(pool.clone())
        .latest(Decimal::ZERO)
        .await
        .unwrap();

    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        (
            snapshots[0].price_a_fetched_at,
            snapshots[0].price_b_fetched_at
        ),
        (fetched_at, fetched_at)
    );
}

#[sqlx::test]
async fn incomparable_pools_are_absent(pool: PgPool) {
    let now = Utc::now();

    // Pool 1: complete except it has never swapped (no sqrt_price).
    let no_swap = pk(1).to_string();
    let (mint_a1, mint_b1) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &no_swap, &mint_a1, &mint_b1).await;
    insert_metadata(&pool, &mint_a1, 6, now).await;
    insert_metadata(&pool, &mint_b1, 6, now).await;
    price_at(&pool, &mint_a1, "1.0", now).await;
    price_at(&pool, &mint_b1, "1.0", now).await;
    insert_current_state(&pool, &no_swap, None, None, (0, 0)).await;

    // Pool 2: complete except token B has no price observation.
    let unpriced = pk(4).to_string();
    let (mint_a2, mint_b2) = (pk(5).to_string(), pk(6).to_string());
    insert_pool(&pool, &unpriced, &mint_a2, &mint_b2).await;
    insert_metadata(&pool, &mint_a2, 6, now).await;
    insert_metadata(&pool, &mint_b2, 6, now).await;
    price_at(&pool, &mint_a2, "1.0", now).await;
    insert_current_state(
        &pool,
        &unpriced,
        Some("18446744073709551616"),
        Some(now),
        (0, 0),
    )
    .await;

    let repo = PgPoolPriceSnapshotRepository::new(pool.clone());
    assert!(repo.latest(Decimal::ZERO).await.unwrap().is_empty());
}

#[sqlx::test]
async fn only_pools_deep_enough_at_their_last_prices_are_read(pool: PgPool) {
    // Mutations: `>` for `>=` drops the pool at the floor; the floor read from `pool_current_tvl`
    // drops the deep pool, whose TVL goes NULL there on its 2-hour-old prices.
    let now = Utc::now().trunc_subsecs(6);
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    let (stale_a, stale_b) = (pk(6).to_string(), pk(7).to_string());
    for (mint, decimals) in [(&mint_a, 6), (&mint_b, 9), (&stale_a, 6), (&stale_b, 6)] {
        insert_metadata(&pool, mint, decimals, now).await;
    }
    price_at(&pool, &mint_b, "100.0", now - Duration::minutes(1)).await;
    price_at(&pool, &mint_a, "2.0", now - Duration::minutes(1)).await;
    price_at(&pool, &stale_a, "1.0", now - Duration::hours(2)).await;
    price_at(&pool, &stale_b, "1.0", now - Duration::hours(2)).await;

    // B at $100 and 9 decimals: 100 B is exactly the $10 000 floor.
    for (seed, a, b, reserves) in [
        (1, &mint_a, &mint_b, (0, 99_999_999_999)),
        (4, &mint_a, &mint_b, (0, 100_000_000_000)),
        (5, &stale_a, &stale_b, (1_000_000_000_000, 0)),
    ] {
        let pool_addr = pk(seed).to_string();
        insert_pool(&pool, &pool_addr, a, b).await;
        insert_current_state(
            &pool,
            &pool_addr,
            Some("18446744073709551616"),
            Some(now),
            reserves,
        )
        .await;
    }

    let snapshots = PgPoolPriceSnapshotRepository::new(pool.clone())
        .latest(Decimal::from(10_000))
        .await
        .unwrap();
    let mut read: Vec<_> = snapshots.iter().map(|s| s.pool_address).collect();
    read.sort();

    assert_eq!(
        read,
        vec![pk(4), pk(5)],
        "$9 999.99 is under the floor, $10 000 is on it, and a $1M pool stays \
         whatever its prices' age — the detector counts it `stale`"
    );
}

#[sqlx::test]
async fn the_floor_values_a_pool_as_pool_current_tvl_does(pool: PgPool) {
    // Mutation: the floor's own arithmetic in place of `yog_amount_usd` (decimals dropped on one
    // side) — the two TVLs part, and the pool falls off its own published TVL.
    let now = Utc::now().trunc_subsecs(6);
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    insert_metadata(&pool, &mint_a, 6, now).await;
    insert_metadata(&pool, &mint_b, 9, now).await;
    price_at(&pool, &mint_a, "2.0", now - Duration::minutes(1)).await;
    price_at(&pool, &mint_b, "100.0", now - Duration::minutes(1)).await;

    let pool_addr = pk(1).to_string();
    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    // 1.5 A at $2 and 2.5 B at $100: $253.
    insert_current_state(
        &pool,
        &pool_addr,
        Some("18446744073709551616"),
        Some(now),
        (1_500_000, 2_500_000_000),
    )
    .await;

    let tvl: Decimal = sqlx::query_scalar("SELECT tvl_usd FROM pool_current_tvl")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(tvl, Decimal::from(253));

    let repo = PgPoolPriceSnapshotRepository::new(pool.clone());
    assert_eq!(repo.latest(tvl).await.unwrap().len(), 1);
    assert!(
        repo.latest(tvl + Decimal::new(1, 6))
            .await
            .unwrap()
            .is_empty(),
        "the floor must read the pool at exactly its published TVL, not above"
    );
}
