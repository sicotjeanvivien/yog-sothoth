//! Integration tests for `reward_v` inside `meteora_damm_v2_pool_hourly_activity`
//! (migration **010**) — the `rewardsClaimedUsd` figure of
//! `GET /api/pools/{address}/history`.
//!
//! Gated behind `integration-tests`. The aggregate underneath groups by
//! `mint_reward` as well as by `(pool, bucket)`, so **a bucket holds one row per
//! reward mint** and its valuation is an aggregate across mints. That is what
//! makes a sub-total possible where the other three CTEs of the view cannot
//! produce one, and it is what these tests pin:
//!
//!   * one mint that paid out and cannot be valued ⇒ the whole bucket is NULL,
//!     never the priced mint's share wearing the pair's name;
//!   * a mint that paid out **nothing** needs no price — zero is its value in
//!     any currency, so it must not poison a bucket that is otherwise known;
//!   * "cannot be valued" covers a missing `token_metadata` row exactly as it
//!     covers a missing price. Before 010 the metadata join was INNER, so that
//!     row *disappeared* — and a row that is gone is one `bool_and` never sees.
//!
//! The aggregate is `materialized_only = false`, so real-time aggregation makes
//! the freshly inserted events visible with no manual refresh (same as
//! `claim_caggs.rs`).

use super::helpers::{claim_reward, pk, price_mint_since};
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

/// The event's own hour is what the aggregate buckets on; two hours back keeps
/// it clear of the current, still-open one.
fn event_time() -> DateTime<Utc> {
    Utc::now() - Duration::hours(2)
}

async fn insert_metadata(pool: &PgPool, mint: &str, decimals: i16) {
    let at = Utc::now() - Duration::days(1);
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

/// `None` = the view has no row for this pool at all; `Some(None)` = the bucket
/// exists and its reward total is unknown. The distinction is the point of
/// `an_unresolved_reward_mint_still_produces_a_bucket`.
///
/// Every fixture here pins its claims to one hour, so the view must hold at
/// most ONE bucket per pool — asserted rather than assumed. `fetch_optional`
/// would have returned the first row and dropped the others in silence, so a
/// fixture that later adds a second hour would read an arbitrary bucket and
/// pass or fail on the wrong one.
async fn rewards_usd(pool: &PgPool, pool_addr: &str) -> Option<Option<f64>> {
    let rows = sqlx::query_as::<_, (Option<f64>,)>(
        "SELECT rewards_claimed_usd::DOUBLE PRECISION
         FROM meteora_damm_v2_pool_hourly_activity
         WHERE pool_address = $1",
    )
    .bind(pool_addr)
    .fetch_all(pool)
    .await
    .unwrap();

    assert!(
        rows.len() <= 1,
        "the fixture put activity in {} distinct hours; this helper reads a \
         single bucket and would be answering about an arbitrary one",
        rows.len()
    );
    rows.into_iter().next().map(|(v,)| v)
}

#[sqlx::test]
async fn mixing_a_priced_and_an_unpriced_mint_yields_null(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (priced, unpriced) = (pk(2).to_string(), pk(3).to_string());
    let at = event_time();

    insert_metadata(&pool, &priced, 6).await;
    insert_metadata(&pool, &unpriced, 6).await;
    // Only the first mint is priced. The second has no price row at all —
    // absence, which no staleness bound reaches.
    price_mint_since(&pool, &priced, "2.0", 6).await;

    // One token of each, same hour, same pool.
    claim_reward(&pool, &pool_addr, "r1", &priced, 0, 1_000_000, at).await;
    claim_reward(&pool, &pool_addr, "r2", &unpriced, 1, 1_000_000, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(None),
        "a bucket holding an unvaluable mint must be NULL — before 010 this \
         returned 2.0, the priced mint's share published as the pair's total"
    );
}

#[sqlx::test]
async fn a_fully_priced_bucket_sums_its_mints(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (mint_x, mint_y) = (pk(2).to_string(), pk(3).to_string());
    let at = event_time();

    insert_metadata(&pool, &mint_x, 6).await;
    insert_metadata(&pool, &mint_y, 6).await;
    price_mint_since(&pool, &mint_x, "2.0", 6).await;
    price_mint_since(&pool, &mint_y, "3.0", 6).await;

    claim_reward(&pool, &pool_addr, "r1", &mint_x, 0, 1_000_000, at).await;
    claim_reward(&pool, &pool_addr, "r2", &mint_y, 1, 1_000_000, at).await;

    // 1 × $2 + 1 × $3. The guard must let through what it is meant to let
    // through: a completeness test that also blocks the complete case would
    // pass the test above while publishing nothing at all.
    let value = rewards_usd(&pool, &pool_addr)
        .await
        .expect("the bucket exists")
        .expect("both mints are valuable");
    assert!((value - 5.0).abs() < 1e-9, "expected 5.0, got {value}");
}

#[sqlx::test]
async fn a_zero_claim_needs_no_price(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint = pk(2).to_string();
    let at = event_time();

    insert_metadata(&pool, &mint, 6).await;
    // Deliberately unpriced: nothing was paid out, so nothing needs valuing.

    claim_reward(&pool, &pool_addr, "r1", &mint, 0, 0, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(Some(0.0)),
        "a claim of zero is worth zero in any currency — the empty leg must not \
         be declared unknown for want of a price"
    );
}

#[sqlx::test]
async fn an_unresolved_reward_mint_yields_null(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let (resolved, unresolved) = (pk(2).to_string(), pk(3).to_string());
    let at = event_time();

    // BOTH mints are priced. Only the first has its decimals resolved, so the
    // second cannot be scaled — `POWER(10, NULL)`. This isolates the metadata
    // mechanism from the price one.
    insert_metadata(&pool, &resolved, 6).await;
    price_mint_since(&pool, &resolved, "2.0", 6).await;
    price_mint_since(&pool, &unresolved, "5.0", 6).await;

    claim_reward(&pool, &pool_addr, "r1", &resolved, 0, 1_000_000, at).await;
    claim_reward(&pool, &pool_addr, "r2", &unresolved, 1, 1_000_000, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(None),
        "an unresolved mint is unvaluable like an unpriced one — before 010 the \
         INNER JOIN dropped its row and 2.0 was published as the pair's total"
    );
}

#[sqlx::test]
async fn an_unresolved_reward_mint_still_produces_a_bucket(pool: PgPool) {
    let pool_addr = pk(1).to_string();
    let mint = pk(2).to_string();
    let at = event_time();

    // No `token_metadata` row, and nothing else happened in this pool: before
    // 010 the INNER JOIN removed the only row, so the hour did not exist in the
    // view and `/history` showed nothing at all for it.
    price_mint_since(&pool, &mint, "5.0", 6).await;
    claim_reward(&pool, &pool_addr, "r1", &mint, 0, 1_000_000, at).await;

    assert_eq!(
        rewards_usd(&pool, &pool_addr).await,
        Some(None),
        "the hour must appear with an unknown total — \"it was claimed, we could \
         not price it\", not \"nothing happened\""
    );
}
