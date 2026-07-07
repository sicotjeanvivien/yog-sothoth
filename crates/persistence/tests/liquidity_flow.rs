//! Integration test for the liquidity flow view (migration 025) and the
//! `PgLiquidityFlowRepository` read path.
//!
//! Gated behind `integration-tests`. Validates the vertical slice: liquidity
//! events land in the raw hypertable, the hourly CA (011) exposes the
//! per-direction sums, `meteora_damm_v2_pool_hourly_liquidity_flow` values
//! each direction (both token legs) at the per-bucket trade-time price, and
//! the repository sums the window and joins the pool's current TVL
//! (`pool_current_tvl`, nullable).

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;

use yog_core::domain::LiquidityFlowRepository;
use yog_persistence::PgLiquidityFlowRepository;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

async fn insert_liquidity_event(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    kind: &str,
    amount_a: i64,
    amount_b: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b,
            liquidity_delta, reserve_a_after, reserve_b_after, position, owner, timestamp)
         VALUES ($1,$2,$3,$4,$5,0,0,0,'','',$6)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(kind)
    .bind(amount_a)
    .bind(amount_b)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

/// Seed one pool (token A: 6 decimals @ $2, token B: 9 decimals @ $100)
/// and return its address string.
async fn seed_pool(pool: &PgPool, price_at: DateTime<Utc>) -> String {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

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
        .bind(price_at)
        .execute(pool)
        .await
        .unwrap();
    }

    for (mint, price) in [(&mint_a, "2.0"), (&mint_b, "100.0")] {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
             VALUES ($1,$2::NUMERIC,'jupiter',$3)",
        )
        .bind(mint)
        .bind(price)
        .bind(price_at)
        .execute(pool)
        .await
        .unwrap();
    }

    pool_addr
}

fn close(got: Decimal, want: i64) -> bool {
    (got - Decimal::from(want)).abs() < Decimal::new(1, 4)
}

#[sqlx::test]
async fn flows_split_directions_window_and_join_tvl(pool: PgPool) {
    let now = Utc::now();
    let price_at = now - Duration::hours(9);
    let pool_addr = seed_pool(&pool, price_at).await;

    // Current state: reserves 10.0 A (6 dec) and 1.0 B (9 dec)
    //   → TVL = 10 × $2 + 1 × $100 = $120.
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b)
         VALUES ($1,'meteora_damm_v2',$2,'liquidity_add','sig',10000000,1000000000)",
    )
    .bind(&pool_addr)
    .bind(now)
    .execute(&pool)
    .await
    .unwrap();

    // In-window adds: 2.0 A ($4) + 0.1 B ($10) → added_usd = $14.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_add1",
        "add",
        2_000_000,
        100_000_000,
        now - Duration::hours(2),
    )
    .await;
    // In-window removes over two events:
    //   3.0 A ($6) + 0.5 B ($50), then 1.0 A ($2) → removed_usd = $58.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_rem1",
        "remove",
        3_000_000,
        500_000_000,
        now - Duration::hours(1),
    )
    .await;
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_rem2",
        "remove",
        1_000_000,
        0,
        now - Duration::hours(3),
    )
    .await;
    // Outside the window — must be excluded.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_old",
        "remove",
        999_000_000,
        0,
        now - Duration::hours(30),
    )
    .await;

    let repo = PgLiquidityFlowRepository::new(pool.clone());
    let flows = repo
        .liquidity_flow_since(now - Duration::hours(6))
        .await
        .unwrap();

    let flow = flows
        .iter()
        .find(|f| f.pool_address == pk(1))
        .expect("pool with priced liquidity events must be present");

    assert!(
        close(flow.added_usd, 14),
        "added expected ~$14, got {}",
        flow.added_usd
    );
    assert!(
        close(flow.removed_usd, 58),
        "removed expected ~$58, got {}",
        flow.removed_usd
    );
    let tvl = flow.tvl_usd.expect("TVL must be valued");
    assert!(close(tvl, 120), "TVL expected ~$120, got {tvl}");
}

#[sqlx::test]
async fn pool_without_current_state_has_null_tvl(pool: PgPool) {
    let now = Utc::now();
    let pool_addr = seed_pool(&pool, now - Duration::hours(9)).await;

    // Liquidity movement but NO pool_current_state row (claim-only pool):
    // the flow must surface with tvl_usd = None, not vanish.
    insert_liquidity_event(
        &pool,
        &pool_addr,
        "sig_rem",
        "remove",
        1_000_000,
        0,
        now - Duration::hours(1),
    )
    .await;

    let repo = PgLiquidityFlowRepository::new(pool.clone());
    let flows = repo
        .liquidity_flow_since(now - Duration::hours(6))
        .await
        .unwrap();

    let flow = flows
        .iter()
        .find(|f| f.pool_address == pk(1))
        .expect("flow must be present even without a TVL");
    assert!(close(flow.removed_usd, 2), "removed expected ~$2");
    assert_eq!(flow.tvl_usd, None);
}
