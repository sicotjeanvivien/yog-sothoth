//! The directional swap flow view and `PgSwapFlowRepository`: each direction valued at the
//! bucket's trade-time price on its input side, without collapsing them, summed over the window.

use super::helpers::{as_role, pk, price_mint_since};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::SwapFlowRepository;
use yog_persistence::PgSwapFlowRepository;

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

    // ⚠️ Sampled hourly: the as-of lookup reads one hour back, so a lone row prices one bucket.
    for (mint, price) in [(&mint_a, "2.0"), (&mint_b, "100.0")] {
        price_mint_since(&pool, mint, price, 4).await;
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
    let a_to_b = flow
        .volume_a_to_b_usd
        .expect("every bucket in the window is valuable, so the sum is a total");
    let b_to_a = flow
        .volume_b_to_a_usd
        .expect("every bucket in the window is valuable, so the sum is a total");
    assert!(close(a_to_b, 6), "a_to_b expected ~$6, got {a_to_b}");
    assert!(close(b_to_a, 200), "b_to_a expected ~$200, got {b_to_a}");
}

/// A pool with token A unpriced and token B priced over `priced_hours`: a fresh token
/// against SOL.
async fn seed_pool_with_unpriced_a(pool: &PgPool, priced_hours: i64) -> String {
    let pool_addr = pk(1).to_string();
    let (mint_a, mint_b) = (pk(2).to_string(), pk(3).to_string());
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&pool_addr)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(pool)
    .await
    .unwrap();

    for (mint, decimals) in [(&mint_a, 6i16), (&mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1,$2,$3,$3)",
        )
        .bind(mint)
        .bind(decimals)
        .bind(now - Duration::hours(48))
        .execute(pool)
        .await
        .unwrap();
    }

    price_mint_since(pool, &mint_b, "100.0", priced_hours).await;

    pool_addr
}

#[sqlx::test]
async fn a_partly_unvaluable_window_yields_no_total(pool: PgPool) {
    // Mutation: no `bool_and` — `SUM` skips the NULL bucket and publishes a sub-total.
    let now = Utc::now();
    let pool_addr = seed_pool_with_unpriced_a(&pool, 2).await;

    for (sig, hours_ago) in [("sig_new", 1), ("sig_old", 6)] {
        insert_swap(
            &pool,
            &pool_addr,
            sig,
            "b_to_a",
            0,
            1_000_000_000,
            now - Duration::hours(hours_ago),
        )
        .await;
    }

    let repo = PgSwapFlowRepository::new(pool.clone());
    let flows = repo
        .directional_volume_since(now - Duration::hours(24))
        .await
        .unwrap();
    let flow = flows.iter().find(|f| f.pool_address == pk(1)).unwrap();

    assert_eq!(
        flow.volume_b_to_a_usd, None,
        "one unvaluable hour makes the whole total unknown — publishing the \
         $100 of the valuable hour would understate a $200 window, silently"
    );
    assert_eq!(
        flow.volume_a_to_b_usd, None,
        "both directions go absent together, never one without the other"
    );
}

#[sqlx::test]
async fn an_empty_direction_is_zero_not_unknown(pool: PgPool) {
    // Mutation: no empty-leg `CASE`. ⚠️ It takes a missing decimal, not a missing price:
    // an unpriced A still gets an implied rate, and `0 * rate` is 0 without the CASE.
    let now = Utc::now();
    let pool_addr = seed_pool_with_unpriced_a(&pool, 8).await;

    sqlx::query("DELETE FROM token_metadata WHERE mint = $1")
        .bind(pk(2).to_string())
        .execute(&pool)
        .await
        .unwrap();

    // b_to_a with the fee on B: `a_to_b` paying on A exists in no fee mode.
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1,'sig_one_way','b_to_a',500000,2000000000,0,0,0,2500000,0,0,0,false,$2,0,0)",
    )
    .bind(&pool_addr)
    .bind(now - Duration::hours(2))
    .execute(&pool)
    .await
    .unwrap();

    let repo = PgSwapFlowRepository::new(pool.clone());
    let flows = repo
        .directional_volume_since(now - Duration::hours(24))
        .await
        .unwrap();
    let flow = flows.iter().find(|f| f.pool_address == pk(1)).unwrap();

    assert_eq!(
        flow.volume_a_to_b_usd,
        Some(Decimal::ZERO),
        "no A ever entered the pool this hour: that direction is zero in any \
         currency, known without A's price OR its decimals"
    );
    let b_to_a = flow
        .volume_b_to_a_usd
        .expect("the B side carries everything and B is priced");
    let close = |got: Decimal, want: i64| (got - Decimal::from(want)).abs() < Decimal::new(1, 4);
    assert!(close(b_to_a, 200), "2.0 B at $100 is $200, got {b_to_a}");
}

/// Against the grant on `token_prices` lost: the pricing function reads it as the
/// caller, not as the view's owner, so `yog_signals` needs its own SELECT.
#[sqlx::test]
async fn yog_signals_values_the_flow_under_its_own_role(pool: PgPool) {
    let now = Utc::now();
    let pool_addr = seed_pool_with_unpriced_a(&pool, 4).await;
    insert_swap(
        &pool,
        &pool_addr,
        "sig_signals",
        "b_to_a",
        0,
        1_000_000_000,
        now - Duration::hours(1),
    )
    .await;

    let repo = PgSwapFlowRepository::new(as_role(&pool, "yog_signals").await);
    let flows = repo
        .directional_volume_since(now - Duration::hours(24))
        .await
        .unwrap();
    let flow = flows.iter().find(|f| f.pool_address == pk(1)).unwrap();

    assert_eq!(flow.volume_b_to_a_usd, Some(Decimal::from(100)));
}
