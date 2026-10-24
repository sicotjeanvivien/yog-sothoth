//! The liquidity facts of `PgAnalyticRepository` (035): each move valued once at its pool's
//! price in the tranche — bounded by what the pool traded, the market capping it in a tranche
//! without a swap — and summed per protocol and hour from the first hour valued whole.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{pk, price_at, setup_roles_and_connect_as, sqrt_price_x64};

/// The first tranche of the hour three hours back: settled, and the start of its hour.
fn settled_hour() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

/// A pool of `token` (6 decimals) against SOL (9 decimals).
async fn setup_pool(pool: &PgPool, seed: u8, token: &str, sol: &str) -> String {
    let address = pk(seed).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(token)
    .bind(sol)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals) in [(token, 6i16), (sol, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, now(), now())
             ON CONFLICT (mint) DO NOTHING",
        )
        .bind(mint)
        .bind(decimals)
        .execute(pool)
        .await
        .unwrap();
    }
    address
}

/// A swap of `a` token units for `b` lamports, without fee.
async fn insert_swap(pool: &PgPool, address: &str, tag: &str, a: i64, b: i64, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'a_to_b', $3, $4, 0, 0, $5, 0, 0, 0, 0, false, $6, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(a)
    .bind(b)
    .bind(sqrt_price_x64(a, b))
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_move(
    pool: &PgPool,
    address: &str,
    tag: &str,
    kind: &str,
    (a, b): (i64, i64),
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_liquidity_events
           (pool_address, signature, liquidity_event_kind, amount_a, amount_b, liquidity_delta,
            reserve_a_after, reserve_b_after, position, owner, timestamp, slot, event_index)
         VALUES ($1, $2, $3, $4, $5, 0, 0, 0, 'position', 'owner', $6, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(kind)
    .bind(a)
    .bind(b)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// Values the tranches from `first` through `last`, at the first instant the last one may be.
async fn value(pool: &PgPool, first: DateTime<Utc>, last: DateTime<Utc>) {
    let progress = AnalyticProgress {
        first_hour: Some(first.duration_trunc(Duration::hours(1)).unwrap()),
        last_frozen: None,
        last_tranche: Some(first - Duration::minutes(5)),
    };
    PgAnalyticRepository::new(setup_roles_and_connect_as(pool, "yog_analytic").await)
        .write_cycle(&CycleWrite::plan(last + Duration::minutes(15), progress))
        .await
        .unwrap();
}

async fn move_usd(pool: &PgPool, tag: &str) -> Option<BigDecimal> {
    sqlx::query_scalar("SELECT amount_usd FROM analytic_liquidity_moves WHERE signature = $1")
        .bind(tag)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Token X trades at 0.0001 SOL in its pool while its market says $1; SOL is at $100 and is the
/// anchor, traded in a second pool. Returns the pool of X.
async fn setup_inflated_token(pool: &PgPool, tranche: DateTime<Utc>) -> String {
    let (x, y, sol) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let pool_x = setup_pool(pool, 1, &x, &sol).await;
    let pool_y = setup_pool(pool, 2, &y, &sol).await;
    price_at(pool, &x, "1", tranche - Duration::minutes(1)).await;
    price_at(pool, &sol, "100", tranche - Duration::minutes(1)).await;
    insert_swap(pool, &pool_x, "x", 1_000_000, 100_000, tranche).await;
    insert_swap(pool, &pool_y, "y", 1_000_000, 1_000_000, tranche).await;
    pool_x
}

/// Mutation: the move valued at market, as the flow view does — 10 X at $1 would make it $20.
#[sqlx::test]
async fn a_move_in_a_tranche_with_a_swap_takes_the_pools_bounded_price(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let pool_x = setup_inflated_token(&pool, tranche).await;
    insert_move(
        &pool,
        &pool_x,
        "add",
        "add",
        (10_000_000, 100_000_000),
        tranche,
    )
    .await;

    value(&pool, tranche, tranche).await;

    assert_eq!(move_usd(&pool, "add").await, Some(dec("10.1")));
}

/// Mutation: the last bounded price taken without the market's cap — $0.01 instead of $0.005.
#[sqlx::test]
async fn a_move_without_a_swap_takes_the_lower_of_its_market_and_the_last_bounded_price(
    pool: PgPool,
) {
    let tranche = settled_hour() + Duration::minutes(10);
    let next = tranche + Duration::minutes(5);
    let pool_x = setup_inflated_token(&pool, tranche).await;
    let (x, y_pool) = (pk(10).to_string(), pk(2).to_string());
    // ⚠️ After the first tranche closes: before, its bounded price would already be $0.005.
    price_at(&pool, &x, "0.005", next + Duration::minutes(1)).await;
    insert_swap(&pool, &y_pool, "y2", 1_000_000, 1_000_000, next).await;
    insert_move(
        &pool,
        &pool_x,
        "remove",
        "remove",
        (10_000_000, 100_000_000),
        next,
    )
    .await;

    value(&pool, tranche, next).await;

    assert_eq!(move_usd(&pool, "remove").await, Some(dec("10.05")));
}

/// Mutation: a pool never priced valued anyway, at market or at its mid price.
#[sqlx::test]
async fn a_move_in_a_pool_never_priced_is_not_valued(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    setup_inflated_token(&pool, tranche).await;
    let (z, sol) = (pk(13).to_string(), pk(12).to_string());
    let pool_z = setup_pool(&pool, 3, &z, &sol).await;
    price_at(&pool, &z, "2", tranche - Duration::minutes(1)).await;
    insert_move(
        &pool,
        &pool_z,
        "new",
        "add",
        (1_000_000, 1_000_000_000),
        tranche,
    )
    .await;

    value(&pool, tranche, tranche).await;

    assert_eq!(move_usd(&pool, "new").await, None);
}

async fn hourly(pool: &PgPool) -> Vec<(DateTime<Utc>, BigDecimal, BigDecimal, i64, i64)> {
    sqlx::query_as(
        "SELECT bucket, added_usd, removed_usd, moves, moves_valued
           FROM analytic_protocol_liquidity_hourly ORDER BY bucket",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Mutation: the start dropped — the hour before, its tranches valued without their moves,
/// gets a row of zeros that reads as an hour without any.
#[sqlx::test]
async fn the_hours_sum_their_moves_from_the_first_hour_valued_whole(pool: PgPool) {
    let hour = settled_hour();
    let late = hour - Duration::minutes(10);
    let pool_x = setup_inflated_token(&pool, hour).await;
    let pool_z = setup_pool(&pool, 3, &pk(13).to_string(), &pk(12).to_string()).await;
    insert_move(&pool, &pool_x, "a1", "add", (10_000_000, 100_000_000), hour).await;
    insert_move(&pool, &pool_x, "r1", "remove", (0, 50_000_000), hour).await;
    insert_move(&pool, &pool_z, "z1", "add", (1_000_000, 0), hour).await;

    value(&pool, late, hour).await;

    assert_eq!(
        hourly(&pool).await,
        vec![(hour, dec("10.1"), dec("5"), 3, 2)],
    );
}
