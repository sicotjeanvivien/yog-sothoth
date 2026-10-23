//! The price table of `PgAnalyticRepository` (028, 031): each pool's two tokens priced once per
//! 5-minute tranche — the anchor at its market price, the other bounded by what the tranche
//! traded — each direction valued by its trusted leg, the hours summed, no swap valued elsewhere.

use std::path::Path;

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleOutcome, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{pk, price_at, setup_roles_and_connect_as, sqrt_price_x64};

/// A tranche three hours back, ten minutes into its hour: settled, and clear of
/// the hour's first and last tranches.
fn settled_tranche() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
        + Duration::minutes(10)
}

fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(Duration::hours(1)).unwrap()
}

/// A pool of `(mint, decimals)` for each side.
async fn setup_pool(pool: &PgPool, seed: u8, a: (&str, i16), b: (&str, i16)) -> String {
    let address = pk(seed).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(a.0)
    .bind(b.0)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals) in [a, b] {
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

/// One swap: `amounts` is `(amount_a, amount_b)`, the input and output its direction says.
struct Trade<'a> {
    tag: &'a str,
    direction: &'a str,
    amounts: (i64, i64),
    fee: i64,
    fee_token_is_a: bool,
    sqrt_price: BigDecimal,
    at: DateTime<Utc>,
}

async fn insert_trade(pool: &PgPool, address: &str, trade: &Trade<'_>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1, $2, $3, $4, $5, 0, 0, $6, $7, 0, 0, 0, $8, $9, 0, 0)",
    )
    .bind(address)
    .bind(trade.tag)
    .bind(trade.direction)
    .bind(trade.amounts.0)
    .bind(trade.amounts.1)
    .bind(&trade.sqrt_price)
    .bind(trade.fee)
    .bind(trade.fee_token_is_a)
    .bind(trade.at)
    .execute(pool)
    .await
    .unwrap();
}

/// A swap A into B without fee, the pool left at `sqrt_price`.
async fn insert_swap(
    pool: &PgPool,
    address: &str,
    tag: &str,
    amounts: (i64, i64),
    sqrt_price: BigDecimal,
    at: DateTime<Utc>,
) {
    let a_to_b = Trade {
        tag,
        direction: "a_to_b",
        amounts,
        fee: 0,
        fee_token_is_a: false,
        sqrt_price,
        at,
    };
    insert_trade(pool, address, &a_to_b).await;
}

fn repository_of(pool: PgPool) -> PgAnalyticRepository {
    PgAnalyticRepository::new(pool)
}

/// The cycle at `now` of a writer that valued every tranche before `tranche`.
async fn run_cycle_at(pool: &PgPool, tranche: DateTime<Utc>, now: DateTime<Utc>) -> CycleOutcome {
    let progress = AnalyticProgress {
        first_hour: Some(hour_of(tranche)),
        last_frozen: None,
        last_tranche: Some(tranche - Duration::minutes(5)),
    };
    repository_of(setup_roles_and_connect_as(pool, "yog_analytic").await)
        .write_cycle(&CycleWrite::plan(now, progress))
        .await
        .unwrap()
}

/// Values `tranche` alone, at the first instant it may be.
async fn value(pool: &PgPool, tranche: DateTime<Utc>) -> CycleOutcome {
    run_cycle_at(pool, tranche, tranche + Duration::minutes(15)).await
}

type Range = (
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
);

/// `(market_a, price_a, market_b, price_b)` of the pool in the tranche.
async fn prices(pool: &PgPool, address: &str, tranche: DateTime<Utc>) -> Range {
    sqlx::query_as(
        "SELECT market_a_usd, price_a_usd, market_b_usd, price_b_usd
           FROM analytic_pool_prices_5min WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(address)
    .bind(tranche)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

/// Against the bound dropped, or the sale valued by IA's leg: IA would keep its market price
/// of before its fall, and its 1.94 billion units sold be worth $14.5 million.
#[sqlx::test]
async fn a_token_emptied_in_its_pool_is_worth_what_the_pool_pays(pool: PgPool) {
    let tranche = settled_tranche();
    let (ia, sol, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let emptied = setup_pool(&pool, 1, (&ia, 6), (&sol, 9)).await;
    let beside = setup_pool(&pool, 2, (&other, 6), (&sol, 9)).await;
    price_at(
        &pool,
        &ia,
        "0.00744765469211184",
        tranche - Duration::minutes(1),
    )
    .await;
    price_at(&pool, &sol, "107.28", tranche - Duration::minutes(1)).await;
    insert_swap(
        &pool,
        &emptied,
        "ia",
        (1_944_522_520_695_913, 4_850_552),
        dec("394034525786492"),
        tranche + Duration::minutes(1),
    )
    .await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1_000, 1_000),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;

    value(&pool, tranche).await;

    let (market_ia, price_ia, market_sol, price_sol) = prices(&pool, &emptied, tranche).await;
    assert_eq!(market_ia, Some(dec("0.00744765469211184")));
    assert_eq!(
        (market_sol, price_sol),
        (Some(dec("107.28")), Some(dec("107.28")))
    );
    let price_ia = price_ia.unwrap();
    assert!(
        price_ia > dec("0.0000000001") && price_ia < dec("0.000000001"),
        "IA is worth what its pool pays, ~2.7e-10 $: {price_ia}"
    );
    let volume: Option<BigDecimal> = sqlx::query_scalar(
        "SELECT volume_in_a_usd FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&emptied)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        volume.unwrap() < dec("1"),
        "less than a dollar changed hands"
    );
}

/// Against the anchor bounded as well, on either side of its pool: SOL would take a
/// market price too low through the pool's rate; and against the other side left unbounded.
#[sqlx::test]
async fn the_anchor_keeps_its_market_price_on_either_side(pool: PgPool) {
    let tranche = settled_tranche();
    let sol = pk(30).to_string();
    let (m, n, o) = (pk(31).to_string(), pk(32).to_string(), pk(33).to_string());
    price_at(&pool, &sol, "100", tranche).await;
    // SOL as B, 0.25 SOL for an M: M is worth $25 in the pool, $0.001 on the market.
    let sol_as_b = setup_pool(&pool, 1, (&m, 6), (&sol, 6)).await;
    price_at(&pool, &m, "0.001", tranche).await;
    insert_swap(&pool, &sol_as_b, "m", (4, 1), sqrt_price_x64(4, 1), tranche).await;
    // SOL as A, 4 N or 4 O for a SOL: each is worth $25 in its pool.
    let low_beside = setup_pool(&pool, 2, (&sol, 6), (&n, 6)).await;
    price_at(&pool, &n, "0.001", tranche).await;
    insert_swap(
        &pool,
        &low_beside,
        "n",
        (1, 4),
        sqrt_price_x64(1, 4),
        tranche,
    )
    .await;
    let high_beside = setup_pool(&pool, 3, (&sol, 6), (&o, 6)).await;
    price_at(&pool, &o, "50", tranche).await;
    insert_swap(
        &pool,
        &high_beside,
        "o",
        (1, 4),
        sqrt_price_x64(1, 4),
        tranche,
    )
    .await;

    value(&pool, tranche).await;

    let (_, price_m, _, sol_b) = prices(&pool, &sol_as_b, tranche).await;
    assert_eq!((price_m, sol_b), (Some(dec("0.001")), Some(dec("100"))));
    let (_, sol_a, _, price_n) = prices(&pool, &low_beside, tranche).await;
    assert_eq!((sol_a, price_n), (Some(dec("100")), Some(dec("0.001"))));
    let (_, sol_a, _, price_o) = prices(&pool, &high_beside, tranche).await;
    assert_eq!((sol_a, price_o), (Some(dec("100")), Some(dec("25"))));
}

/// Against a side taken for the anchor on a tie, A or B: it would keep a market price
/// its pool contradicts.
#[sqlx::test]
async fn with_no_anchor_each_token_takes_the_lower_of_its_two_prices(pool: PgPool) {
    let tranche = settled_tranche();
    let mut found = Vec::new();
    // 0.25 B for an A in both pools; A over-priced by the market in the first, B in the second.
    for (seed, market_a, market_b) in [(1u8, "30", "100"), (2, "2", "100")] {
        let (a, b) = (pk(seed + 10).to_string(), pk(seed + 20).to_string());
        let address = setup_pool(&pool, seed, (&a, 6), (&b, 6)).await;
        price_at(&pool, &a, market_a, tranche).await;
        price_at(&pool, &b, market_b, tranche).await;
        insert_swap(
            &pool,
            &address,
            &format!("ab{seed}"),
            (4, 1),
            sqrt_price_x64(4, 1),
            tranche,
        )
        .await;
        found.push(address);
    }

    value(&pool, tranche).await;

    let (_, a_over, _, b_beside) = prices(&pool, &found[0], tranche).await;
    assert_eq!((a_over, b_beside), (Some(dec("25")), Some(dec("100"))));
    let (_, a_beside, _, b_over) = prices(&pool, &found[1], tranche).await;
    assert_eq!((a_beside, b_over), (Some(dec("2")), Some(dec("8"))));
}

/// Against `LEAST` turned into a strict minimum: a token Jupiter does not price would
/// lose its value through the pool, as most fresh tokens against SOL would.
#[sqlx::test]
async fn a_token_without_a_market_price_takes_its_price_in_the_pool(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &b, "100", tranche).await;
    insert_swap(&pool, &address, "ab", (4, 1), sqrt_price_x64(4, 1), tranche).await;

    value(&pool, tranche).await;

    let (market_a, price_a, _, price_b) = prices(&pool, &address, tranche).await;
    assert_eq!(market_a, None);
    assert_eq!((price_a, price_b), (Some(dec("25")), Some(dec("100"))));
}

/// Against the valued flag read from one side only: a tranche with no price at all
/// would publish a zero.
#[sqlx::test]
async fn a_tranche_without_any_price_is_not_valued(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    insert_swap(&pool, &address, "ab", (4, 1), sqrt_price_x64(4, 1), tranche).await;

    value(&pool, tranche).await;

    let valued: (bool, Option<BigDecimal>) = sqlx::query_as(
        "SELECT valued, volume_in_a_usd FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(valued, (false, None));
}

/// `(valued, volume_in_a_usd, volume_in_b_usd)` of the pool's one tranche.
async fn valued_tranche(
    pool: &PgPool,
    address: &str,
) -> (bool, Option<BigDecimal>, Option<BigDecimal>) {
    sqlx::query_as(
        "SELECT valued, volume_in_a_usd, volume_in_b_usd
           FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(address)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// A pool whose `token_metadata` row is gone: its decimals are unknown.
async fn forget_decimals(pool: &PgPool, mint: &str) {
    sqlx::query("DELETE FROM token_metadata WHERE mint = $1")
        .bind(mint)
        .execute(pool)
        .await
        .unwrap();
}

/// Against the pool's mid price taken for the rate: WIGGA, without a market price, would be
/// worth 21 876 SOL for what its empty pool traded at 2.4e-7 SOL, and its 3 swaps $801 million.
#[sqlx::test]
async fn an_empty_pool_is_worth_what_it_traded(pool: PgPool) {
    let tranche = settled_tranche();
    let (wigga, sol, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let empty = setup_pool(&pool, 1, (&wigga, 6), (&sol, 9)).await;
    let beside = setup_pool(&pool, 2, (&other, 6), (&sol, 9)).await;
    price_at(&pool, &sol, "104", tranche - Duration::minutes(1)).await;
    let mid = sqrt_price_x64(1, 21_876_000);
    for (tag, direction, amounts) in [
        ("sell", "a_to_b", (352_231_421, 58_808)),
        ("buy", "b_to_a", (352_231_420, 110_000)),
    ] {
        let at = tranche + Duration::minutes(1);
        let swap = Trade {
            tag,
            direction,
            amounts,
            fee: 0,
            fee_token_is_a: false,
            sqrt_price: mid.clone(),
            at,
        };
        insert_trade(&pool, &empty, &swap).await;
    }
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1_000, 1),
        sqrt_price_x64(1_000, 1),
        tranche,
    )
    .await;

    value(&pool, tranche).await;

    let (market_wigga, price_wigga, ..) = prices(&pool, &empty, tranche).await;
    assert_eq!(market_wigga, None);
    let price_wigga = price_wigga.unwrap();
    assert!(
        price_wigga > dec("0.00002") && price_wigga < dec("0.00003"),
        "WIGGA is worth what its pool traded, ~2.5e-5 $: {price_wigga}"
    );
    let (valued, sells, buys) = valued_tranche(&pool, &empty).await;
    assert!(valued);
    assert!(
        sells.unwrap() + buys.unwrap() < dec("0.02"),
        "less than two cents changed hands"
    );
}

/// Against a direction valued by its whole column (`traded_b`), or by the amount times the
/// tranche's rate: the sale of a token that fell within the tranche would be worth 1.1 or 0.55 SOL.
#[sqlx::test]
async fn each_direction_is_worth_its_trusted_leg(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    // 1 SOL buys 1 000 A, then 1 000 A sell for 0.1 SOL.
    for (tag, direction, amounts) in [
        ("buy", "b_to_a", (1_000_000_000, 1_000_000)),
        ("sell", "a_to_b", (1_000_000_000, 100_000)),
    ] {
        let swap = Trade {
            tag,
            direction,
            amounts,
            fee: 0,
            fee_token_is_a: false,
            sqrt_price: sqrt_price_x64(10_000, 1),
            at: tranche,
        };
        insert_trade(&pool, &address, &swap).await;
    }

    value(&pool, tranche).await;

    let (valued, sells, buys) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(
        (sells, buys),
        (Some(dec("10")), Some(dec("100"))),
        "the sale is worth the 0.1 SOL it got, the purchase the 1 SOL it paid"
    );
}

/// Against the anchor's leg taken without its price: X, in more pools than Y but without a
/// market price, would leave its pool unvalued where Y's leg says what changed hands.
#[sqlx::test]
async fn the_trusted_leg_is_the_priced_one(pool: PgPool) {
    let tranche = settled_tranche();
    let (x, y, z) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let xy = setup_pool(&pool, 1, (&x, 6), (&y, 6)).await;
    let xz = setup_pool(&pool, 2, (&x, 6), (&z, 6)).await;
    price_at(&pool, &y, "2", tranche).await;
    insert_swap(
        &pool,
        &xy,
        "xy",
        (10_000_000, 5_000_000),
        sqrt_price_x64(2, 1),
        tranche,
    )
    .await;
    insert_swap(&pool, &xz, "xz", (1, 1), sqrt_price_x64(1, 1), tranche).await;

    value(&pool, tranche).await;

    let (valued, sells, _) = valued_tranche(&pool, &xy).await;
    assert!(valued);
    assert_eq!(sells, Some(dec("10")), "the 5 Y received, at $2");
}

/// Against both decimals required to value a direction: a fresh token Helius has not
/// described yet would leave unvalued a sale that SOL's leg measures.
#[sqlx::test]
async fn a_token_without_metadata_is_valued_by_its_partner(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    let beside = setup_pool(&pool, 2, (&other, 6), (&sol, 6)).await;
    forget_decimals(&pool, &a).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1, 1),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;
    insert_swap(
        &pool,
        &address,
        "ab",
        (4, 1_000_000),
        sqrt_price_x64(4, 1),
        tranche,
    )
    .await;

    value(&pool, tranche).await;

    let (_, price_a, _, price_sol) = prices(&pool, &address, tranche).await;
    assert_eq!(
        (price_a, price_sol),
        (None, Some(dec("100"))),
        "without the tranche's rate, only SOL has a price"
    );
    let (valued, sells, _) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(sells, Some(dec("100")), "the 1 SOL received, at $100");
}

/// `(tag, direction, (amount_a, amount_b), fee, fee_token_is_a)`.
type Swap = (&'static str, &'static str, (i64, i64), i64, bool);

/// `swaps` at `at`; the pool's mid price is not read.
async fn insert_trades(pool: &PgPool, address: &str, swaps: &[Swap], at: DateTime<Utc>) {
    for &(tag, direction, amounts, fee, fee_token_is_a) in swaps {
        let swap = Trade {
            tag,
            direction,
            amounts,
            fee,
            fee_token_is_a,
            sqrt_price: sqrt_price_x64(1, 1),
            at,
        };
        insert_trade(pool, address, &swap).await;
    }
}

/// `fees_usd` of the pool's one tranche.
async fn fees(pool: &PgPool, address: &str) -> Option<BigDecimal> {
    sqlx::query_scalar("SELECT fees_usd FROM analytic_pool_5min WHERE pool_address = $1")
        .bind(address)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Its 5-minute aggregate materialised, then the raw swaps of `address` gone.
async fn drop_the_raw(pool: &PgPool, address: &str, tranche: DateTime<Utc>) {
    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', $1, $2)")
        .bind(tranche)
        .bind(tranche + Duration::minutes(5))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM meteora_damm_v2_swap_events WHERE pool_address = $1")
        .bind(address)
        .execute(pool)
        .await
        .unwrap();
}

/// A sale and a purchase of A against SOL at $100, each paying 0.01 SOL of fee: the purchase
/// on its input, the sale on its output — a pool collecting its fees in B.
const ONLY_B: [Swap; 2] = [
    ("buy", "b_to_a", (990_000_000, 1_000_000), 10_000, false),
    ("sell", "a_to_b", (1_000_000_000, 990_000), 10_000, false),
];

/// Against the sale valued net: the SOL its seller received leave out the fee the pool took
/// on them, and the sale reads a fee short of its trade.
#[sqlx::test]
async fn a_sale_counts_its_fee(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    let sale = [("sell", "a_to_b", (1_000_000_000, 990_000), 10_000, false)];
    insert_trades(&pool, &address, &sale, tranche).await;

    value(&pool, tranche).await;

    let (valued, sells, _) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(
        sells,
        Some(dec("100")),
        "0.99 SOL received + 0.01 SOL of fee"
    );
    assert_eq!(fees(&pool, &address).await, Some(dec("1")));
}

/// Against the B fee of a pool collecting B both ways given to its sales alone: they would
/// count the purchase's input fee as well, one fee more than they traded.
#[sqlx::test]
async fn an_only_b_pool_splits_its_fees_by_direction(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_trades(&pool, &address, &ONLY_B, tranche).await;

    value(&pool, tranche).await;

    let (valued, sells, buys) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(
        (sells, buys),
        (Some(dec("100")), Some(dec("100"))),
        "each direction is its gross trade, 1 SOL"
    );
    assert_eq!(fees(&pool, &address).await, Some(dec("2")));
}

/// Against the split read back from the raw swaps: their retention gone, a two-way tranche of
/// a pool collecting B both ways would lose it, though its aggregate still holds every fee.
#[sqlx::test]
async fn the_split_outlives_the_raw(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_trades(&pool, &address, &ONLY_B, tranche).await;
    drop_the_raw(&pool, &address, tranche).await;

    value(&pool, tranche).await;

    assert_eq!(
        valued_tranche(&pool, &address).await,
        (true, Some(dec("100")), Some(dec("100")))
    );
    assert_eq!(fees(&pool, &address).await, Some(dec("2")));
}

/// ☠️ Pins the loss the READMEs warn about: a purchase landing after its tranche was
/// materialised is left out of it whole, its fee with it, and the tranche stays valued.
#[sqlx::test]
async fn a_swap_late_past_the_aggregate_is_left_out_whole(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_trades(&pool, &address, &ONLY_B, tranche).await;
    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', $1, $2)")
        .bind(tranche)
        .bind(tranche + Duration::minutes(5))
        .execute(&pool)
        .await
        .unwrap();
    let late = [(
        "late",
        "b_to_a",
        (9_900_000_000, 10_000_000),
        100_000,
        false,
    )];
    insert_trades(&pool, &address, &late, tranche + Duration::minutes(1)).await;

    value(&pool, tranche).await;

    assert_eq!(
        valued_tranche(&pool, &address).await,
        (true, Some(dec("100")), Some(dec("100"))),
        "the tranche as its aggregate holds it, the late swap left out"
    );
}

/// Against the B fees priced at one rate for both directions: X fell between its sale and its
/// purchase, and the purchase's referral fee would take the sale's higher price.
#[sqlx::test]
async fn protocol_and_referral_fees_take_their_direction_price(pool: PgPool) {
    let tranche = settled_tranche();
    let (sol, x, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let address = setup_pool(&pool, 1, (&sol, 6), (&x, 6)).await;
    let beside = setup_pool(&pool, 2, (&sol, 6), (&other, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1, 1),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;
    // 1 SOL sold for 1 000 X gross, 10 X the fee; then 10 000 X in, 100 X of fee, buy 0.99 SOL.
    let swaps = [
        (
            "sell",
            "a_to_b",
            (1_000_000, 990_000_000),
            10_000_000,
            false,
        ),
        (
            "buy",
            "b_to_a",
            (990_000, 10_000_000_000),
            100_000_000,
            false,
        ),
    ];
    insert_trades(&pool, &address, &swaps, tranche).await;
    sqlx::query(
        "UPDATE meteora_damm_v2_swap_events
            SET claiming_fee = claiming_fee - 40000000, referral_fee = 40000000
          WHERE signature = 'buy'",
    )
    .execute(&pool)
    .await
    .unwrap();

    value(&pool, tranche).await;

    let (fees, referral): (Option<BigDecimal>, Option<BigDecimal>) = sqlx::query_as(
        "SELECT fees_usd, referral_fees_usd FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        fees,
        Some(dec("2")),
        "10 of the sale's 1 000 X, 100 of the buy's 10 000 X"
    );
    assert_eq!(
        referral,
        Some(dec("0.4")),
        "40 of the buy's 10 000 X, at $100 the SOL"
    );
}

/// Against a sale's B fee shared by its net output: SOL sold for X, the fee taken in X on
/// the way out, would be worth 1/990 of the trade instead of the 10 X of 1 000 it took.
#[sqlx::test]
async fn a_sale_of_the_trusted_input_pays_its_output_fee_share(pool: PgPool) {
    let tranche = settled_tranche();
    let (sol, x, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let address = setup_pool(&pool, 1, (&sol, 6), (&x, 6)).await;
    let beside = setup_pool(&pool, 2, (&sol, 6), (&other, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1, 1),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;
    // 1 SOL in for 990 X out, 10 X more the fee.
    let sale = [(
        "sell",
        "a_to_b",
        (1_000_000, 990_000_000),
        10_000_000,
        false,
    )];
    insert_trades(&pool, &address, &sale, tranche).await;

    value(&pool, tranche).await;

    let (valued, sells, _) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(sells, Some(dec("100")), "the 1 SOL paid in");
    assert_eq!(
        fees(&pool, &address).await,
        Some(dec("1")),
        "10 of the 1 000 X"
    );
}

/// Against the trusted output taken net of the input fee: SOL bought with X, its fee taken
/// in X on the way in, would be worth the 0.99 SOL received, not the 1 SOL it traded.
#[sqlx::test]
async fn a_sale_of_the_trusted_output_gives_back_its_input_fee(pool: PgPool) {
    let tranche = settled_tranche();
    let (sol, x, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let address = setup_pool(&pool, 1, (&sol, 6), (&x, 6)).await;
    let beside = setup_pool(&pool, 2, (&sol, 6), (&other, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1, 1),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;
    // 1 000 X in, 10 X of it the fee, for 0.99 SOL out.
    let buy = [("buy", "b_to_a", (990_000, 1_000_000_000), 10_000_000, false)];
    insert_trades(&pool, &address, &buy, tranche).await;

    value(&pool, tranche).await;

    let (valued, _, buys) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(buys, Some(dec("100")), "0.99 SOL × 1 000 X ÷ 990 X");
    assert_eq!(
        fees(&pool, &address).await,
        Some(dec("1")),
        "10 of the 1 000 X"
    );
}

/// Against the input's share divided by nothing: a dust purchase of 1 unit taken whole as fee,
/// for nothing out, would leave its tranche unvalued, and a real sale with it.
#[sqlx::test]
async fn a_dust_purchase_taken_whole_as_fee_is_worth_nothing(pool: PgPool) {
    let tranche = settled_tranche();
    let (sol, x, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let address = setup_pool(&pool, 1, (&sol, 6), (&x, 6)).await;
    let beside = setup_pool(&pool, 2, (&sol, 6), (&other, 6)).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1, 1),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;
    let swaps = [
        ("sell", "a_to_b", (1_000_000, 4_000_000), 0, false),
        ("dust", "b_to_a", (0, 1), 1, false),
    ];
    insert_trades(&pool, &address, &swaps, tranche).await;

    value(&pool, tranche).await;

    assert_eq!(
        valued_tranche(&pool, &address).await,
        (true, Some(dec("100")), Some(dec("0")))
    );
}

/// Against the fee valued at the table's price, the tranche's two directions averaged: bought
/// after the crash, its 100 X would be worth $0.93 instead of the $0.01 they were paid.
#[sqlx::test]
async fn a_fee_in_the_untrusted_token_is_its_share_of_its_trade(pool: PgPool) {
    let tranche = settled_tranche();
    let (x, sol) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&x, 6), (&sol, 9)).await;
    price_at(&pool, &sol, "100", tranche).await;
    // 1 000 X dumped for 1 SOL, then 0.01 SOL buys 9 900 X, 100 X more the fee.
    let crash = [
        ("dump", "a_to_b", (1_000_000_000, 1_000_000_000), 0, false),
        (
            "buy",
            "b_to_a",
            (9_900_000_000, 10_000_000),
            100_000_000,
            true,
        ),
    ];
    insert_trades(&pool, &address, &crash, tranche).await;

    value(&pool, tranche).await;

    assert_eq!(fees(&pool, &address).await, Some(dec("0.01")));
}

/// Against the decimals of the fee's token required: a fee taken in a token Helius has not
/// described yet would leave unvalued a share that needs only its amounts.
#[sqlx::test]
async fn a_fee_in_a_token_without_decimals_is_valued(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, sol, other) = (pk(10).to_string(), pk(11).to_string(), pk(12).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&sol, 6)).await;
    let beside = setup_pool(&pool, 2, (&other, 6), (&sol, 6)).await;
    forget_decimals(&pool, &a).await;
    price_at(&pool, &sol, "100", tranche).await;
    insert_swap(
        &pool,
        &beside,
        "beside",
        (1, 1),
        sqrt_price_x64(1, 1),
        tranche,
    )
    .await;
    // 1 SOL buys 990 units of A, 10 more the fee.
    let buy = [("buy", "b_to_a", (990, 1_000_000), 10, true)];
    insert_trades(&pool, &address, &buy, tranche).await;

    value(&pool, tranche).await;

    assert_eq!(
        valued_tranche(&pool, &address).await,
        (true, Some(dec("0")), Some(dec("100")))
    );
    assert_eq!(fees(&pool, &address).await, Some(dec("1")));
}

/// Against the higher leg taken on a tie: A, priced $30 by the market where its pool pays $25,
/// would value its sale at $120 for the $100 of B it got.
#[sqlx::test]
async fn a_tie_takes_the_lower_gross_leg(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &a, "30", tranche).await;
    price_at(&pool, &b, "100", tranche).await;
    let sale = [("sell", "a_to_b", (4_000_000, 1_000_000), 0, false)];
    insert_trades(&pool, &address, &sale, tranche).await;

    value(&pool, tranche).await;

    let (valued, sells, _) = valued_tranche(&pool, &address).await;
    assert!(valued);
    assert_eq!(sells, Some(dec("100")));
}

/// Against a trusted token's fees read from the price table: on a tie whose other token has
/// no decimals, the table has no price for USDC, and a fee in it would leave the tranche unvalued.
#[sqlx::test]
async fn a_trusted_token_prices_its_fees_at_market_on_a_tie(pool: PgPool) {
    let tranche = settled_tranche();
    let (x, usdc) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&x, 6), (&usdc, 6)).await;
    forget_decimals(&pool, &x).await;
    price_at(&pool, &usdc, "1", tranche).await;
    let sale = [("sell", "a_to_b", (1_000, 990_000), 10_000, false)];
    insert_trades(&pool, &address, &sale, tranche).await;

    value(&pool, tranche).await;

    let (_, _, _, price_usdc) = prices(&pool, &address, tranche).await;
    assert_eq!(price_usdc, None, "the table has no price for USDC here");
    assert_eq!(
        valued_tranche(&pool, &address).await,
        (true, Some(dec("1")), Some(dec("0")))
    );
    assert_eq!(fees(&pool, &address).await, Some(dec("0.01")));
}

/// Against the margin dropped from the tranche's close: a swap landing within it would
/// miss a tranche already valued.
#[sqlx::test]
async fn a_tranche_waits_ten_minutes_after_its_close(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &b, "100", tranche).await;
    insert_swap(&pool, &address, "ab", (4, 1), sqrt_price_x64(4, 1), tranche).await;

    let early = run_cycle_at(&pool, tranche, tranche + Duration::minutes(14)).await;

    assert_eq!(early.tranches_written, 0);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM analytic_pool_prices_5min")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

/// ☠️ Pins the loss the READMEs warn about: a swap that lands in a tranche already
/// valued stays out of it, and its tranche is never written twice.
#[sqlx::test]
async fn a_tranche_is_valued_once(pool: PgPool) {
    let tranche = settled_tranche();
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &b, "100", tranche).await;
    insert_swap(
        &pool,
        &address,
        "on_time",
        (4, 1),
        sqrt_price_x64(4, 1),
        tranche,
    )
    .await;
    value(&pool, tranche).await;

    insert_swap(
        &pool,
        &address,
        "late",
        (4, 1),
        sqrt_price_x64(4, 1),
        tranche,
    )
    .await;
    let analytic = repository_of(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    let progress = analytic.progress().await.unwrap();
    analytic
        .write_cycle(&CycleWrite::plan(tranche + Duration::minutes(30), progress))
        .await
        .unwrap();

    let swaps: i64 = sqlx::query_scalar(
        "SELECT swap_count FROM analytic_pool_5min WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(&address)
    .bind(tranche)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(swaps, 1);
}

/// Against the open hour summed short of its tranches, or a column summed from another.
/// ⚠️ The open hour ends the cycle's range: a bound that stops at its start leaves it empty.
#[sqlx::test]
async fn an_hourly_fact_is_the_sum_of_its_tranches(pool: PgPool) {
    let hour = hour_of(settled_tranche());
    let (a, b) = (pk(10).to_string(), pk(11).to_string());
    let address = setup_pool(&pool, 1, (&a, 6), (&b, 6)).await;
    price_at(&pool, &a, "30", hour).await;
    price_at(&pool, &b, "100", hour).await;
    for (minute, amounts) in [(5, (4, 1)), (20, (8, 2)), (50, (40, 10))] {
        let at = hour + Duration::minutes(minute) + Duration::seconds(30);
        insert_swap(
            &pool,
            &address,
            &format!("m{minute}"),
            amounts,
            sqrt_price_x64(4, 1),
            at,
        )
        .await;
    }

    let progress = AnalyticProgress {
        first_hour: Some(hour),
        last_frozen: None,
        last_tranche: None,
    };
    repository_of(setup_roles_and_connect_as(&pool, "yog_analytic").await)
        .write_cycle(&CycleWrite::plan(hour + Duration::minutes(40), progress))
        .await
        .unwrap();

    let sums: (
        i64,
        Option<BigDecimal>,
        Option<BigDecimal>,
        Option<BigDecimal>,
    ) = sqlx::query_as(
        "SELECT SUM(swap_count)::BIGINT, SUM(volume_in_a_usd), SUM(volume_in_b_usd),
                    SUM(fees_usd)
               FROM analytic_pool_5min WHERE pool_address = $1",
    )
    .bind(&address)
    .fetch_one(&pool)
    .await
    .unwrap();
    let fact: (
        i64,
        Option<BigDecimal>,
        Option<BigDecimal>,
        Option<BigDecimal>,
    ) = sqlx::query_as(
        "SELECT swap_count, volume_in_a_usd, volume_in_b_usd, fees_usd
               FROM analytic_pool_hourly WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(&address)
    .bind(hour)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sums.0, 2, "the tranche of :50 is not closed yet");
    assert_eq!(fact, sums);
}

/// The views the persistence code reads.
async fn views_read_by_a_binary(pool: &PgPool) -> Vec<String> {
    let views: Vec<String> =
        sqlx::query_scalar("SELECT viewname::TEXT FROM pg_views WHERE schemaname = 'public'")
            .fetch_all(pool)
            .await
            .unwrap();
    let sources = sources_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
    views
        .into_iter()
        .filter(|view| sources.iter().any(|(_, text)| text.contains(view.as_str())))
        .collect()
}

/// `(path, text)` of every Rust and SQL file under `dir`.
fn sources_under(dir: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(sources_under(&path));
        } else if path.extension().is_some_and(|e| e == "rs" || e == "sql") {
            let text = std::fs::read_to_string(&path).unwrap();
            files.push((path.display().to_string(), text));
        }
    }
    files
}

/// The views of `views` that reach, through other views, the swaps valued by the hour.
async fn reaching_the_hourly_swap_valuation(pool: &PgPool, views: &[String]) -> Vec<String> {
    sqlx::query_scalar(
        "WITH RECURSIVE dep (view, rel) AS (
             SELECT v.oid, d.refobjid
               FROM pg_class v
               JOIN pg_rewrite r ON r.ev_class = v.oid
               JOIN pg_depend d ON d.objid = r.oid AND d.classid = 'pg_rewrite'::regclass
                               AND d.refclassid = 'pg_class'::regclass
              WHERE v.relname = ANY($1) AND d.refobjid <> v.oid
             UNION
             SELECT dep.view, d.refobjid
               FROM dep
               JOIN pg_rewrite r ON r.ev_class = dep.rel
               JOIN pg_depend d ON d.objid = r.oid AND d.classid = 'pg_rewrite'::regclass
                               AND d.refclassid = 'pg_class'::regclass
              WHERE d.refobjid <> dep.rel
         )
         SELECT DISTINCT v.relname::TEXT
           FROM dep JOIN pg_class v ON v.oid = dep.view JOIN pg_class c ON c.oid = dep.rel
          WHERE c.relname = 'meteora_damm_v2_swap_events_hourly_priced'",
    )
    .bind(views)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn no_view_a_binary_reads_values_a_swap_by_the_hour(pool: PgPool) {
    let views = views_read_by_a_binary(&pool).await;
    assert!(
        views
            .iter()
            .any(|v| v == "meteora_damm_v2_pool_hourly_flow"),
        "the scan must reach the flow the signals read, got {views:?}"
    );

    assert_eq!(
        reaching_the_hourly_swap_valuation(&pool, &views).await,
        Vec::<String>::new()
    );
}

/// The guard above, against the flow pointed back at the hourly valuation.
#[sqlx::test]
async fn the_guard_catches_a_flow_valued_by_the_hour(pool: PgPool) {
    sqlx::query(
        "CREATE OR REPLACE VIEW meteora_damm_v2_pool_hourly_flow AS
         SELECT pool_address, bucket,
                yog_moved_amount_usd(volume_in_a, dec_a, eff_price_a) AS volume_a_to_b_usd,
                yog_moved_amount_usd(volume_in_b, dec_b, eff_price_b) AS volume_b_to_a_usd,
                valuation_complete
           FROM meteora_damm_v2_swap_events_hourly_priced",
    )
    .execute(&pool)
    .await
    .unwrap();

    let views = views_read_by_a_binary(&pool).await;
    assert_eq!(
        reaching_the_hourly_swap_valuation(&pool, &views).await,
        ["meteora_damm_v2_pool_hourly_flow"]
    );
}

/// Against a market price read in the writer outside the price table: a swap valued there
/// would escape the bound.
#[test]
fn the_writer_reads_market_prices_in_the_price_table_alone() {
    let writer = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/repositories");
    let readers: Vec<String> = sources_under(&writer)
        .into_iter()
        .filter(|(path, _)| path.contains("analytic"))
        .filter(|(_, text)| {
            [
                "token_prices",
                "yog_price_asof",
                "swap_hour_prices",
                "hourly_priced",
            ]
            .iter()
            .any(|reader| text.contains(reader))
        })
        .map(|(path, _)| path)
        .collect();

    assert_eq!(readers.len(), 1, "{readers:?}");
    assert!(readers[0].ends_with("meteora_damm_v2_tranche.sql"));
}
