//! Integration test for the `meteora_damm_v2_liquidity_events_valued` VIEW
//! (migration 021) — the per-event trade-time USD valuation behind the
//! pool-detail liquidity table's "Value (USD)" column.
//!
//! Gated behind `integration-tests`. Asserts two things on real SQL: the value
//! uses the price *as-of the event* (not a later price), and it is NULL when a
//! leg has no price as-of the event (rather than fabricating a partial value).

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

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

#[allow(clippy::too_many_arguments)]
async fn insert_liquidity(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    amount_a: i64,
    amount_b: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind,
            amount_a, amount_b, liquidity_delta, reserve_a_after, reserve_b_after,
            position, owner, timestamp)
         VALUES ($1,$2,'add',$3,$4,0::NUMERIC,0,0,'pos','own',$5)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(amount_a)
    .bind(amount_b)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

async fn value_usd(pool: &PgPool, signature: &str) -> Option<f64> {
    let v: Option<f64> = sqlx::query_scalar(
        "SELECT value_usd::DOUBLE PRECISION
         FROM meteora_damm_v2_liquidity_events_valued
         WHERE signature = $1",
    )
    .bind(signature)
    .fetch_one(pool)
    .await
    .unwrap();
    v
}

#[sqlx::test]
async fn values_both_legs_at_the_as_of_price(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    let event_at = Utc::now() - Duration::hours(1);

    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    // SOL-like (9 dec) and USDC-like (6 dec).
    insert_metadata(&pool, &mint_a, 9, event_at - Duration::days(1)).await;
    insert_metadata(&pool, &mint_b, 6, event_at - Duration::days(1)).await;

    // Price as-of the event: A = $71.5, B = $1.0. A LATER price for A ($999)
    // must be ignored — the valuation is trade-time, not current.
    insert_price(&pool, &mint_a, "71.5", event_at - Duration::hours(2)).await;
    insert_price(&pool, &mint_a, "999.0", event_at + Duration::hours(2)).await;
    insert_price(&pool, &mint_b, "1.0", event_at - Duration::hours(2)).await;

    // 5 SOL (5e9 @ 9 dec) + 100 USDC (100e6 @ 6 dec)
    //   = 5 × 71.5 + 100 × 1.0 = 457.5
    insert_liquidity(
        &pool,
        &pool_addr,
        "evt1",
        5_000_000_000,
        100_000_000,
        event_at,
    )
    .await;

    let value = value_usd(&pool, "evt1")
        .await
        .expect("value should compute");
    assert!(
        (value - 457.5).abs() < 1e-6,
        "expected 457.5 (as-of price), got {value}"
    );
}

#[sqlx::test]
async fn value_is_null_when_a_leg_has_no_price(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    let event_at = Utc::now() - Duration::hours(1);

    insert_pool(&pool, &pool_addr, &mint_a, &mint_b).await;
    insert_metadata(&pool, &mint_a, 9, event_at - Duration::days(1)).await;
    insert_metadata(&pool, &mint_b, 6, event_at - Duration::days(1)).await;

    // Only token A is priced; token B has no price row → value unknown.
    insert_price(&pool, &mint_a, "71.5", event_at - Duration::hours(2)).await;

    insert_liquidity(
        &pool,
        &pool_addr,
        "evt2",
        5_000_000_000,
        100_000_000,
        event_at,
    )
    .await;

    assert_eq!(
        value_usd(&pool, "evt2").await,
        None,
        "an unpriced leg must yield NULL, not a partial value"
    );
}
