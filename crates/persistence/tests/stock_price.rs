//! `token_stock_price` (045): a stock is valued at the market price unless it is more than
//! `yog_market_over_pool_max()` times the highest price our pools' trades gave the token, its
//! ceiling; read through `pool_current_tvl`, whose sides then count 0.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::helpers::{pk, price_at};

/// A pool `seed` of A and B holding 10 A and 1 B (6 and 9 decimals).
async fn seed_pool(pool: &PgPool, seed: u8, mint_a: &str, mint_b: &str) -> String {
    let address = pk(seed).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(mint_a)
    .bind(mint_b)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals) in [(mint_a, 6i16), (mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, now(), now()) ON CONFLICT (mint) DO NOTHING",
        )
        .bind(mint)
        .bind(decimals)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_slot, last_event_index)
         VALUES ($1, 'meteora_damm_v2', now(), 'swap', 'sig', 10000000, 1000000000, 1, 0)",
    )
    .bind(&address)
    .execute(pool)
    .await
    .unwrap();
    address
}

/// The two mints of the tests, A and B.
fn mints() -> (String, String) {
    (pk(20).to_string(), pk(21).to_string())
}

async fn ceiling(pool: &PgPool, mint: &str, usd: &str) {
    sqlx::query(
        "INSERT INTO analytic_token_pool_ceilings (mint, ceiling_usd) VALUES ($1, $2::NUMERIC)",
    )
    .bind(mint)
    .bind(usd)
    .execute(pool)
    .await
    .unwrap();
}

async fn price_now(pool: &PgPool, mint: &str, price: &str) {
    price_at(pool, mint, price, Utc::now() - Duration::minutes(1)).await;
}

type Sides = (
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
    Option<Decimal>,
);

/// `(value_a_usd, value_b_usd, tvl_usd, tvl_complete_usd)` of `address`.
async fn sides(pool: &PgPool, address: &str) -> Sides {
    sqlx::query_as(
        "SELECT value_a_usd, value_b_usd, tvl_usd, tvl_complete_usd
           FROM pool_current_tvl WHERE pool_address = $1",
    )
    .bind(address)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn usd(value: &str) -> Option<Decimal> {
    Some(value.parse().unwrap())
}

/// Against the check dropped from the view: A quoted 100 000 times what our pools gave it would
/// count $2 000 000; and `tvl_complete_usd`, which `tvl_drain` reads, goes NULL with it.
#[sqlx::test]
async fn a_market_price_our_pools_contradict_counts_zero(pool: PgPool) {
    let (a, b) = mints();
    let address = seed_pool(&pool, 1, &a, &b).await;
    ceiling(&pool, &a, "2").await;
    price_now(&pool, &a, "200000").await;
    price_now(&pool, &b, "100").await;

    assert_eq!(
        sides(&pool, &address).await,
        (None, usd("100"), usd("100"), None)
    );
}

/// Against the threshold moved to 1 (the 9 times refused) or to 100 (the 11 times believed), and
/// against `>=` for `>` (exactly 10 times refused).
#[sqlx::test]
async fn a_market_price_up_to_ten_times_the_ceiling_is_believed(pool: PgPool) {
    let (a, b) = mints();
    let address = seed_pool(&pool, 1, &a, &b).await;
    ceiling(&pool, &a, "2").await;
    price_now(&pool, &b, "100").await;

    price_now(&pool, &a, "18").await;
    assert_eq!(
        sides(&pool, &address).await,
        (usd("180"), usd("100"), usd("280"), usd("280"))
    );

    price_now(&pool, &a, "20").await;
    assert_eq!(
        sides(&pool, &address).await,
        (usd("200"), usd("100"), usd("300"), usd("300"))
    );

    price_now(&pool, &a, "22").await;
    assert_eq!(
        sides(&pool, &address).await,
        (None, usd("100"), usd("100"), None)
    );
}

/// Against the ceiling read as required: a token no pool of ours traded keeps its market price.
#[sqlx::test]
async fn without_a_ceiling_the_market_price_stands(pool: PgPool) {
    let (a, b) = mints();
    let address = seed_pool(&pool, 1, &a, &b).await;
    price_now(&pool, &a, "200000").await;
    price_now(&pool, &b, "100").await;

    assert_eq!(
        sides(&pool, &address).await,
        (usd("2000000"), usd("100"), usd("2000100"), usd("2000100"))
    );
}

/// Against a check both ways: A fallen to a hundredth of its ceiling is worth that, and B keeps
/// its value.
#[sqlx::test]
async fn a_token_that_fell_keeps_its_market_price(pool: PgPool) {
    let (a, b) = mints();
    let address = seed_pool(&pool, 1, &a, &b).await;
    ceiling(&pool, &a, "2").await;
    ceiling(&pool, &b, "100").await;
    price_now(&pool, &a, "0.02").await;
    price_now(&pool, &b, "100").await;

    assert_eq!(
        sides(&pool, &address).await,
        (usd("0.2"), usd("100"), usd("100.2"), usd("100.2"))
    );
}

/// The ceiling is the token's, not a pool's: a second pool of A, which never traded, is held to it
/// as the first is. Against the check keyed on the pool.
#[sqlx::test]
async fn the_ceiling_holds_every_pool_of_the_token(pool: PgPool) {
    let (a, b) = mints();
    let traded = seed_pool(&pool, 1, &a, &b).await;
    let quiet = seed_pool(&pool, 2, &a, &b).await;
    ceiling(&pool, &a, "2").await;
    price_now(&pool, &a, "200000").await;
    price_now(&pool, &b, "100").await;

    for address in [traded, quiet] {
        assert_eq!(
            sides(&pool, &address).await,
            (None, usd("100"), usd("100"), None)
        );
    }
}

/// The 15-minute bound still holds under the ceiling. Against it dropped from the view.
#[sqlx::test]
async fn a_stale_price_values_nothing(pool: PgPool) {
    let (a, b) = mints();
    let address = seed_pool(&pool, 1, &a, &b).await;
    ceiling(&pool, &a, "2").await;
    price_at(&pool, &a, "2", Utc::now() - Duration::minutes(30)).await;
    price_now(&pool, &b, "100").await;

    assert_eq!(
        sides(&pool, &address).await,
        (None, usd("100"), usd("100"), None)
    );
}
