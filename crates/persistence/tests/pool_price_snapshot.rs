//! Integration test for the pool price snapshot view (migration 024) and
//! the `PgPoolPriceSnapshotRepository` read path.
//!
//! Gated behind `integration-tests`. Validates the vertical slice: a pool
//! with resolved mints, a swap-bearing current state and a price for both
//! tokens comes back as one complete snapshot; a pool missing any input
//! (no swap yet, or an unpriced token) is absent rather than half-populated.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, SubsecRound, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::{PoolPriceSnapshotRepository, Protocol};
use yog_persistence::PgPoolPriceSnapshotRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

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

async fn insert_price(pool: &PgPool, mint: &str, price: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
         VALUES ($1,$2::NUMERIC,'jupiter',$3)",
    )
    .bind(mint)
    .bind(price)
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
) {
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_sqrt_price, last_swap_at)
         VALUES ($1,'meteora_damm_v2',NOW(),'swap','sig',0,0,$2::NUMERIC,$3)",
    )
    .bind(pool_addr)
    .bind(sqrt_price)
    .bind(swap_at)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn snapshot_joins_state_decimals_and_latest_prices(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();
    // TIMESTAMPTZ is microsecond-precision; truncate so the round-trip
    // compares exactly.
    let now = Utc::now().trunc_subsecs(6);
    let swap_at = now - Duration::minutes(5);

    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    insert_metadata(&pool, &mint_a, 6, now).await;
    insert_metadata(&pool, &mint_b, 9, now).await;
    // Two observations per mint — the view must pick the most recent one.
    insert_price(&pool, &mint_a, "1.5", now - Duration::hours(2)).await;
    insert_price(&pool, &mint_a, "2.0", now - Duration::minutes(3)).await;
    insert_price(&pool, &mint_b, "80.0", now - Duration::hours(2)).await;
    insert_price(&pool, &mint_b, "100.0", now - Duration::minutes(2)).await;
    // sqrt_price = 2^64 (raw price 1.0 before decimal rescaling).
    insert_current_state(
        &pool,
        &pool_addr,
        Some("18446744073709551616"),
        Some(swap_at),
    )
    .await;

    let repo = PgPoolPriceSnapshotRepository::new(pool.clone());
    let snapshots = repo.latest().await.unwrap();

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

#[sqlx::test]
async fn incomparable_pools_are_absent(pool: PgPool) {
    let now = Utc::now();

    // Pool 1: complete except it has never swapped (no sqrt_price).
    let no_swap = pk(1).to_string();
    let (mint_a1, mint_b1) = (pk(2).to_string(), pk(3).to_string());
    insert_pool(&pool, &no_swap, &mint_a1, &mint_b1).await;
    insert_metadata(&pool, &mint_a1, 6, now).await;
    insert_metadata(&pool, &mint_b1, 6, now).await;
    insert_price(&pool, &mint_a1, "1.0", now).await;
    insert_price(&pool, &mint_b1, "1.0", now).await;
    insert_current_state(&pool, &no_swap, None, None).await;

    // Pool 2: complete except token B has no price observation.
    let unpriced = pk(4).to_string();
    let (mint_a2, mint_b2) = (pk(5).to_string(), pk(6).to_string());
    insert_pool(&pool, &unpriced, &mint_a2, &mint_b2).await;
    insert_metadata(&pool, &mint_a2, 6, now).await;
    insert_metadata(&pool, &mint_b2, 6, now).await;
    insert_price(&pool, &mint_a2, "1.0", now).await;
    insert_current_state(&pool, &unpriced, Some("18446744073709551616"), Some(now)).await;

    let repo = PgPoolPriceSnapshotRepository::new(pool.clone());
    assert!(repo.latest().await.unwrap().is_empty());
}
