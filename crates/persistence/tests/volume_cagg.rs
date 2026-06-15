//! Integration test for the swap volume continuous aggregate (migration 010)
//! and the CA-backed `volume_24h_usd` read path in `PgPoolAnalyticsRepository`.
//!
//! Gated behind `integration-tests`. Validates the full vertical slice:
//! swaps land in the raw hypertable, the hourly CA (real-time aggregation,
//! so no manual refresh needed) exposes the per-direction sums, and
//! `batch_compute` values them at the per-bucket trade-time price — summing
//! only the INPUT side of each swap (a_to_b → amount_a, b_to_a → amount_b),
//! over the trailing 24h window.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::PoolAnalyticsRepository;
use yog_persistence::PgPoolAnalyticsRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

#[allow(clippy::too_many_arguments)]
async fn insert_swap(
    pool: &PgPool,
    pool_addr: &str,
    mint_a: &str,
    mint_b: &str,
    signature: &str,
    direction: &str,
    amount_a: i64,
    amount_b: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, token_a_mint, token_b_mint, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a, timestamp)
         VALUES ($1,$2,$3,$4,$5,$6,$7,0,0,0,0,0,0,0,false,$8)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(mint_a)
    .bind(mint_b)
    .bind(direction)
    .bind(amount_a)
    .bind(amount_b)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn volume_24h_reads_cagg_with_trade_time_pricing(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();
    let now = Utc::now();
    // Prices fetched well before the swap buckets so the as-of lookup hits them.
    let price_at = now - Duration::hours(3);

    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&pool_addr)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(&pool)
    .await
    .unwrap();

    // Token A has 6 decimals, token B has 9.
    for (mint, decimals) in [(&mint_a, 6i16), (&mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1,$2,$3,$3)",
        )
        .bind(mint)
        .bind(decimals)
        .bind(price_at)
        .execute(&pool)
        .await
        .unwrap();
    }

    // Token A = $2.0, token B = $100.0.
    for (mint, price) in [(&mint_a, "2.0"), (&mint_b, "100.0")] {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
             VALUES ($1,$2::NUMERIC,'jupiter',$3)",
        )
        .bind(mint)
        .bind(price)
        .bind(price_at)
        .execute(&pool)
        .await
        .unwrap();
    }

    // a_to_b: input side is amount_a = 1_000_000 (1.0 @ 6 dec) → 1.0 × $2  = $2
    insert_swap(
        &pool,
        &pool_addr,
        &mint_a,
        &mint_b,
        "sig_a",
        "a_to_b",
        1_000_000,
        0,
        now - Duration::hours(1),
    )
    .await;
    // b_to_a: input side is amount_b = 2_000_000_000 (2.0 @ 9 dec) → 2.0 × $100 = $200
    insert_swap(
        &pool,
        &pool_addr,
        &mint_a,
        &mint_b,
        "sig_b",
        "b_to_a",
        0,
        2_000_000_000,
        now - Duration::hours(1),
    )
    .await;
    // Outside the 24h window — must be excluded from the total.
    insert_swap(
        &pool,
        &pool_addr,
        &mint_a,
        &mint_b,
        "sig_old",
        "a_to_b",
        999_000_000,
        0,
        now - Duration::hours(30),
    )
    .await;

    let repo = PgPoolAnalyticsRepository::new(pool.clone());
    let result = repo.batch_compute(&[pk(1)]).await.unwrap();

    let analytics = result.get(&pk(1)).expect("requested pool must be present");
    let volume = analytics
        .volume_24h_usd
        .expect("volume should be Some for a pool with priced swaps in window");

    // $2 (a_to_b) + $200 (b_to_a); the 30h-old swap is excluded.
    let expected = Decimal::from(202);
    assert!(
        (volume - expected).abs() < Decimal::new(1, 4),
        "expected ~{expected}, got {volume}"
    );
}
