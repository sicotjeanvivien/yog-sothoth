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
pub(super) fn settled_hour() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

pub(super) fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

/// A pool of `token` (6 decimals) against SOL (9 decimals).
pub(super) async fn setup_pool(pool: &PgPool, seed: u8, token: &str, sol: &str) -> String {
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
pub(super) async fn insert_swap(
    pool: &PgPool,
    address: &str,
    tag: &str,
    a: i64,
    b: i64,
    at: DateTime<Utc>,
) {
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
pub(super) async fn value(pool: &PgPool, first: DateTime<Utc>, last: DateTime<Utc>) {
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
pub(super) async fn setup_inflated_token(pool: &PgPool, tranche: DateTime<Utc>) -> String {
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

/// Mutation: a pool never priced valued anyway, at market or at its mid price; or an empty move
/// left to the price, read as not valued when it moved nothing.
#[sqlx::test]
async fn a_move_in_a_pool_never_priced_is_not_valued_and_an_empty_one_is_worth_zero(pool: PgPool) {
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
    insert_move(&pool, &pool_z, "empty", "add", (0, 0), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(move_usd(&pool, "new").await, None);
    assert_eq!(move_usd(&pool, "empty").await, Some(dec("0")));
}

/// Mutation: the newest price row taken whatever it holds — its side without a price leaves
/// `LEAST` the market alone, and 10 X would be worth $10 more.
#[sqlx::test]
async fn a_side_the_last_rows_did_not_price_stays_unvalued(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let next = tranche + Duration::minutes(5);
    let (x, y, sol) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let pool_x = setup_pool(&pool, 1, &x, &sol).await;
    let pool_y = setup_pool(&pool, 2, &y, &sol).await;
    price_at(&pool, &x, "1", tranche - Duration::minutes(1)).await;
    price_at(&pool, &sol, "100", tranche - Duration::minutes(1)).await;
    // No SOL out of the swap: no rate, so X gets no price in this tranche.
    insert_swap(&pool, &pool_x, "x", 1_000_000, 0, tranche).await;
    insert_swap(&pool, &pool_y, "y", 1_000_000, 1_000_000, tranche).await;
    insert_swap(&pool, &pool_y, "y2", 1_000_000, 1_000_000, next).await;
    insert_move(&pool, &pool_x, "unbounded", "add", (10_000_000, 0), next).await;

    value(&pool, tranche, next).await;

    assert_eq!(move_usd(&pool, "unbounded").await, None);
}

/// A pool's creation, the way the indexer writes it: `token_a_amount` and `token_b_amount`
/// deposited by `creator`.
async fn insert_creation(
    pool: &PgPool,
    address: &str,
    tag: &str,
    (a, b): (i64, i64),
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_initialize_pool_events
           (pool_address, signature, token_a_mint, token_b_mint, creator, payer, alpha_vault,
            sqrt_min_price, sqrt_max_price, sqrt_price, liquidity, activation_type,
            activation_point, collect_fee_mode, pool_type, token_a_flag, token_b_flag,
            token_a_amount, token_b_amount, total_amount_a, total_amount_b, pool_fees_raw,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'mint_a', 'mint_b', 'creator', 'payer', 'vault', 0, 0, 0, 0, 0, 0, 0, 0,
                 0, 0, $3, $4, $3, $4, ''::bytea, $5, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(a)
    .bind(b)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the creation left out — the pool's first liquidity never counted as entering.
#[sqlx::test]
async fn a_pools_creation_is_a_move_valued_at_its_bounded_price(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let pool_x = setup_inflated_token(&pool, tranche).await;
    insert_creation(&pool, &pool_x, "born", (10_000_000, 100_000_000), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(move_usd(&pool, "born").await, Some(dec("10.1")));
    let kind: String =
        sqlx::query_scalar("SELECT kind FROM analytic_liquidity_moves WHERE signature = 'born'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(kind, "create");
}

/// Mutation: a token without a market price priced by its tranche's trade — one swap at $0.01
/// would make 10 billion units deposited worth $100 million.
#[sqlx::test]
async fn a_token_without_a_market_price_counts_zero_as_in_the_tvl(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let (fwd, y, usdc) = (pk(20).to_string(), pk(21).to_string(), pk(22).to_string());
    let pool_fwd = setup_pool(&pool, 5, &fwd, &usdc).await;
    let pool_y = setup_pool(&pool, 6, &y, &usdc).await;
    price_at(&pool, &usdc, "1", tranche - Duration::minutes(1)).await;
    insert_swap(&pool, &pool_fwd, "tiny", 1_000_000, 10_000_000, tranche).await;
    insert_swap(&pool, &pool_y, "y", 1_000_000, 1_000_000, tranche).await;
    insert_creation(
        &pool,
        &pool_fwd,
        "stock",
        (10_000_000_000_000_000, 0),
        tranche,
    )
    .await;
    insert_move(
        &pool,
        &pool_fwd,
        "both",
        "add",
        (10_000_000_000_000_000, 5_000_000_000),
        tranche,
    )
    .await;

    value(&pool, tranche, tranche).await;

    assert_eq!(move_usd(&pool, "stock").await, None);
    assert_eq!(move_usd(&pool, "both").await, Some(dec("5")));
}

type Hour = (DateTime<Utc>, BigDecimal, BigDecimal, BigDecimal, i64, i64);

async fn hourly(pool: &PgPool) -> Vec<Hour> {
    sqlx::query_as(
        "SELECT bucket, created_usd, added_usd, removed_usd, moves, moves_valued
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
    insert_creation(&pool, &pool_x, "c1", (10_000_000, 100_000_000), hour).await;

    value(&pool, late, hour).await;

    // Mutation: the creations summed with the additions, or left out of the hour.
    assert_eq!(
        hourly(&pool).await,
        vec![(hour, dec("10.1"), dec("10.1"), dec("5"), 4, 3)],
    );
}

async fn insert_vesting_lock(pool: &PgPool, address: &str, signature: &str, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_lock_position_events
           (pool_address, signature, position, owner, vesting, cliff_point, period_frequency,
            cliff_unlock_liquidity, liquidity_per_period, number_of_period, timestamp, slot,
            event_index)
         VALUES ($1, $2, 'position', 'owner', 'vesting', 0, 0, 0, 0, 0, $3, 0, 1)",
    )
    .bind(address)
    .bind(signature)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// A permanent lock of `position` in pool `address`; the moves of these tests hold `position`.
async fn insert_permanent_lock(
    pool: &PgPool,
    (address, position): (&str, &str),
    signature: &str,
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_permanent_lock_position_events
           (pool_address, signature, position, lock_liquidity_amount,
            total_permanent_locked_liquidity, timestamp, slot, event_index)
         VALUES ($1, $2, $3, 0, 0, $4, 0, 2)",
    )
    .bind(address)
    .bind(signature)
    .bind(position)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn move_lock(pool: &PgPool, tag: &str) -> Option<String> {
    sqlx::query_scalar("SELECT lock FROM analytic_liquidity_moves WHERE signature = $1")
        .bind(tag)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Mutation: the lock looked up without its signature, instant, pool or, for an addition, position
/// — another one's lock taken; vesting preferred to permanent; or no lock left `NULL`.
#[sqlx::test]
async fn a_move_says_the_lock_its_transaction_puts_on_a_position(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let pool_x = setup_inflated_token(&pool, tranche).await;
    let at = tranche + Duration::minutes(1);
    for tag in [
        "vesting",
        "permanent",
        "both",
        "none",
        "elsewhere",
        "other-pool",
    ] {
        insert_creation(&pool, &pool_x, tag, (1, 1), at).await;
    }
    for tag in ["add", "add-locked", "add-other-position"] {
        insert_move(&pool, &pool_x, tag, "add", (1, 0), at).await;
    }
    let own = (pool_x.as_str(), "position");
    insert_vesting_lock(&pool, &pool_x, "vesting", at).await;
    insert_permanent_lock(&pool, own, "permanent", at).await;
    insert_vesting_lock(&pool, &pool_x, "both", at).await;
    insert_permanent_lock(&pool, own, "both", at).await;
    insert_vesting_lock(&pool, &pool_x, "another-transaction", at).await;
    insert_permanent_lock(&pool, own, "elsewhere", at + Duration::seconds(30)).await;
    insert_permanent_lock(&pool, ("another-pool", "position"), "other-pool", at).await;
    insert_permanent_lock(&pool, own, "add-locked", at).await;
    insert_permanent_lock(
        &pool,
        (&pool_x, "another-position"),
        "add-other-position",
        at,
    )
    .await;

    value(&pool, tranche, tranche).await;

    for (tag, lock) in [
        ("vesting", "vesting"),
        ("permanent", "permanent"),
        ("both", "permanent"),
        ("none", "none"),
        ("elsewhere", "none"),
        ("other-pool", "none"),
        ("add", "none"),
        ("add-locked", "permanent"),
        ("add-other-position", "none"),
    ] {
        assert_eq!(move_lock(&pool, tag).await.as_deref(), Some(lock), "{tag}");
    }
}
