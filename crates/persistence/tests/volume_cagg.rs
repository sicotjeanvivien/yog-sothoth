//! The swap volume continuous aggregate and `volume_usd` in the activity view: each swap's
//! input side valued at the bucket's trade-time price.

use super::helpers::{pk, price_mint_since, the_only_bucket};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

async fn insert_swap(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    direction: &str,
    amount_a: i64,
    amount_b: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a, timestamp, slot, event_index)
         VALUES ($1,$2,$3,$4,$5,0,0,0,0,0,0,0,false,$6,0,0)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(direction)
    .bind(amount_a)
    .bind(amount_b)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn volume_reads_cagg_with_trade_time_pricing(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();
    let now = Utc::now();
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

    // ⚠️ Sampled hourly: one observation prices only the bucket starting within the hour after it.
    for (mint, price) in [(&mint_a, "2.0"), (&mint_b, "100.0")] {
        price_mint_since(&pool, mint, price, 4).await;
    }

    // a_to_b: input side is amount_a = 1_000_000 (1.0 @ 6 dec) → 1.0 × $2  = $2
    insert_swap(
        &pool,
        &pool_addr,
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
        "sig_b",
        "b_to_a",
        0,
        2_000_000_000,
        now - Duration::hours(1),
    )
    .await;
    let volume = the_only_bucket(&pool, &pool_addr)
        .await
        .volume_usd
        .expect("volume should be Some for a bucket with priced swaps");

    // $2 (a_to_b) + $200 (b_to_a).
    let expected = Decimal::from(202);
    assert!(
        (volume - expected).abs() < Decimal::new(1, 4),
        "expected ~{expected}, got {volume}"
    );
}
