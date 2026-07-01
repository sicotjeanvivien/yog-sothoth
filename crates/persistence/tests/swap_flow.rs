//! Integration test for the directional swap flow view (migration 023) and
//! the `PgSwapFlowRepository` read path.
//!
//! Gated behind `integration-tests`. Validates the vertical slice: swaps land
//! in the raw hypertable, the hourly CA exposes the per-direction sums, and
//! `meteora_damm_v2_pool_hourly_flow` values each direction at the per-bucket
//! trade-time price WITHOUT collapsing them (unlike view 019) — a_to_b priced
//! by token A's input side, b_to_a by token B's — summed over the window.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::SwapFlowRepository;
use yog_persistence::PgSwapFlowRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

#[allow(clippy::too_many_arguments)]
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
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a, timestamp)
         VALUES ($1,$2,$3,$4,$5,0,0,0,0,0,0,0,false,$6)",
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
async fn directional_volume_splits_and_windows(pool: PgPool) {
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

    // Two a_to_b swaps: input side amount_a totals 3_000_000 (3.0 @ 6 dec)
    //   → 3.0 × $2 = $6 of a_to_b flow.
    insert_swap(
        &pool,
        &pool_addr,
        "sig_a1",
        "a_to_b",
        1_000_000,
        0,
        now - Duration::hours(1),
    )
    .await;
    insert_swap(
        &pool,
        &pool_addr,
        "sig_a2",
        "a_to_b",
        2_000_000,
        0,
        now - Duration::hours(2),
    )
    .await;
    // One b_to_a swap: input side amount_b = 2_000_000_000 (2.0 @ 9 dec)
    //   → 2.0 × $100 = $200 of b_to_a flow.
    insert_swap(
        &pool,
        &pool_addr,
        "sig_b1",
        "b_to_a",
        0,
        2_000_000_000,
        now - Duration::hours(1),
    )
    .await;
    // Outside the window — must be excluded.
    insert_swap(
        &pool,
        &pool_addr,
        "sig_old",
        "a_to_b",
        999_000_000,
        0,
        now - Duration::hours(30),
    )
    .await;

    let repo = PgSwapFlowRepository::new(pool.clone());
    let flows = repo
        .directional_volume_since(now - Duration::hours(24))
        .await
        .unwrap();

    let flow = flows
        .iter()
        .find(|f| f.pool_address == pk(1))
        .expect("pool with priced swaps must be present");

    let close = |got: Decimal, want: i64| {
        let want = Decimal::from(want);
        (got - want).abs() < Decimal::new(1, 4)
    };
    assert!(
        close(flow.volume_a_to_b_usd, 6),
        "a_to_b expected ~$6, got {}",
        flow.volume_a_to_b_usd
    );
    assert!(
        close(flow.volume_b_to_a_usd, 200),
        "b_to_a expected ~$200, got {}",
        flow.volume_b_to_a_usd
    );
}
