//! Integration test for the liquidity continuous aggregate (migration 011).
//!
//! Gated behind `integration-tests`. There is no Rust read path consuming this
//! rollup yet (history-only), so the test asserts the CA's per-kind splits
//! directly: `liquidity_delta` is an unsigned magnitude and events carry an
//! add/remove direction, so add and remove must never be summed together.
//! Real-time aggregation means the just-inserted rows are visible without a
//! manual refresh.

#![cfg(feature = "integration-tests")]

use chrono::{DateTime, Duration, Utc};
use solana_pubkey::Pubkey;
use sqlx::PgPool;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

#[allow(clippy::too_many_arguments)]
async fn insert_liquidity(
    pool: &PgPool,
    pool_addr: &str,
    signature: &str,
    kind: &str,
    amount_a: i64,
    amount_b: i64,
    liquidity_delta: i64,
    timestamp: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind,
            amount_a, amount_b, liquidity_delta, reserve_a_after, reserve_b_after,
            position, owner, timestamp)
         VALUES ($1,$2,$3,$4,$5,$6::NUMERIC,0,0,'pos','own',$7)",
    )
    .bind(pool_addr)
    .bind(signature)
    .bind(kind)
    .bind(amount_a)
    .bind(amount_b)
    .bind(liquidity_delta)
    .bind(timestamp)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn liquidity_cagg_splits_add_and_remove(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    // Same hour bucket so the three events collapse into one CA row.
    let ts = Utc::now() - Duration::hours(1);

    // Two adds, one remove.
    insert_liquidity(&pool, &pool_addr, "add1", "add", 100, 200, 1000, ts).await;
    insert_liquidity(&pool, &pool_addr, "add2", "add", 50, 70, 500, ts).await;
    insert_liquidity(&pool, &pool_addr, "rem1", "remove", 30, 40, 300, ts).await;

    let (a_add, b_add, a_rem, b_rem, l_add, l_rem, n_add, n_rem): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT amount_a_added::BIGINT, amount_b_added::BIGINT,
                amount_a_removed::BIGINT, amount_b_removed::BIGINT,
                liquidity_added::BIGINT, liquidity_removed::BIGINT,
                add_count, remove_count
         FROM meteora_damm_v2_liquidity_events_hourly
         WHERE pool_address = $1",
    )
    .bind(&pool_addr)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!((a_add, b_add), (150, 270), "added token amounts");
    assert_eq!((a_rem, b_rem), (30, 40), "removed token amounts");
    assert_eq!((l_add, l_rem), (1500, 300), "liquidity delta split by kind");
    assert_eq!((n_add, n_rem), (2, 1), "event counts split by kind");
}
