//! The realized-fee split of `meteora_damm_v2_pool_hourly_activity`, read back through the
//! repository. ⚠️ The LP share is `claiming + compounding`, not `fees − protocol`: cp-amm takes
//! the referral out of the protocol share. Every fixture number defeats one wrong
//! implementation: four distinct components per side, a fee in each token with different
//! decimals and prices, an unvaluable bucket priced on one side only.

use super::helpers::{pk, price_mint};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::PoolAnalyticsRepository;
use yog_persistence::PgPoolAnalyticsRepository;

// Token scales and prices, different on the two sides.
const DEC_A: i32 = 0;
const DEC_B: i32 = 6;
const PRICE_A: &str = "1.0";
const PRICE_B: &str = "3.0";

// Swap 1: fee in token A (raw = USD: A is $1 with 0 decimals).
const CLAIMING_A: i64 = 70;
const PROTOCOL_A: i64 = 21;
const COMPOUNDING_A: i64 = 4;
const REFERRAL_A: i64 = 5;

// Swap 2: fee in token B (1 000 000 raw = 1.0 B = $3).
const CLAIMING_B: i64 = 700_000;
const PROTOCOL_B: i64 = 210_000;
const COMPOUNDING_B: i64 = 40_000;
const REFERRAL_B: i64 = 50_000;

/// Expected USD figures, as literals: re-deriving them with the code's formula proves nothing.
/// `fees = 103.00`, `protocol = 21.63`, `referral = 5.15`, `lp = 74 + 2.22 = 76.22`
/// — the wrong formula gives 81.37.
fn expected_fees() -> Decimal {
    Decimal::new(10300, 2) // $103.00
}
fn expected_protocol() -> Decimal {
    Decimal::new(2163, 2) // $21.63
}
fn expected_referral() -> Decimal {
    Decimal::new(515, 2) // $5.15
}
fn expected_lp() -> Decimal {
    Decimal::new(7622, 2) // $76.22
}

async fn insert_token(pool: &PgPool, mint: &str, decimals: i32, price: Option<&str>) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1, $2, NOW(), NOW())",
    )
    .bind(mint)
    .bind(decimals)
    .execute(pool)
    .await
    .unwrap();
    if let Some(p) = price {
        price_mint(pool, mint, p).await;
    }
}

async fn insert_pool(pool: &PgPool, addr: &str, mint_a: &str, mint_b: &str) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(addr)
    .bind(mint_a)
    .bind(mint_b)
    .execute(pool)
    .await
    .unwrap();
}

/// One swap, its four fee components and the side they are charged on.
struct Swap<'a> {
    addr: &'a str,
    signature: &'a str,
    direction: &'a str,
    amount_a: i64,
    amount_b: i64,
    claiming: i64,
    protocol: i64,
    compounding: i64,
    referral: i64,
    fee_token_is_a: bool,
    at: DateTime<Utc>,
}

async fn insert_swap(pool: &PgPool, s: Swap<'_>) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1,$2,$3,$4,$5,0,0,0,$6,$7,$8,$9,$10,$11,0,0)",
    )
    .bind(s.addr)
    .bind(s.signature)
    .bind(s.direction)
    .bind(s.amount_a)
    .bind(s.amount_b)
    .bind(s.claiming)
    .bind(s.protocol)
    .bind(s.compounding)
    .bind(s.referral)
    .bind(s.fee_token_is_a)
    .bind(s.at)
    .execute(pool)
    .await
    .unwrap();
}

/// Two swaps in one bucket: one paying its fee in token A, one in token B.
async fn seed_pool_with_both_fee_sides(pool: &PgPool) -> String {
    let addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

    insert_token(pool, &mint_a, DEC_A, Some(PRICE_A)).await;
    insert_token(pool, &mint_b, DEC_B, Some(PRICE_B)).await;
    insert_pool(pool, &addr, &mint_a, &mint_b).await;

    let at = Utc::now() - Duration::hours(1);

    insert_swap(
        pool,
        Swap {
            addr: &addr,
            signature: "sig-fee-in-a",
            direction: "a_to_b",
            amount_a: 1_000,
            amount_b: 2_000_000,
            claiming: CLAIMING_A,
            protocol: PROTOCOL_A,
            compounding: COMPOUNDING_A,
            referral: REFERRAL_A,
            fee_token_is_a: true,
            at,
        },
    )
    .await;

    insert_swap(
        pool,
        Swap {
            addr: &addr,
            signature: "sig-fee-in-b",
            direction: "b_to_a",
            amount_a: 500,
            amount_b: 1_000_000,
            claiming: CLAIMING_B,
            protocol: PROTOCOL_B,
            compounding: COMPOUNDING_B,
            referral: REFERRAL_B,
            fee_token_is_a: false,
            at,
        },
    )
    .await;

    addr
}

#[sqlx::test]
async fn batch_compute_excludes_the_referral_from_the_lp_share(pool: PgPool) {
    let addr = seed_pool_with_both_fee_sides(&pool).await;
    let repo = PgPoolAnalyticsRepository::new(pool);

    let result = repo
        .batch_compute(&[addr.parse().unwrap()])
        .await
        .expect("batch_compute should succeed");
    let a = result
        .get(&addr.parse().unwrap())
        .expect("the seeded pool must be in the batch result");

    assert_eq!(
        a.fees_24h_usd,
        Some(expected_fees()),
        "the total is the sum of the four components — unchanged by this fix"
    );
    assert_eq!(a.protocol_fees_24h_usd, Some(expected_protocol()));
    assert_eq!(
        a.referral_fees_24h_usd,
        Some(expected_referral()),
        "both fee sides must be summed on their own scale and price"
    );
    assert_eq!(
        a.lp_fees_24h_usd,
        Some(expected_lp()),
        "the LP share is claiming + compounding; `fees - protocol` gives {}",
        expected_fees() - expected_protocol()
    );

    assert_eq!(
        a.lp_fees_24h_usd.unwrap()
            + a.protocol_fees_24h_usd.unwrap()
            + a.referral_fees_24h_usd.unwrap(),
        a.fees_24h_usd.unwrap(),
        "the three shares must partition the total exactly"
    );
}

#[sqlx::test]
async fn history_publishes_the_same_split_as_the_24h_analytics(pool: PgPool) {
    let addr = seed_pool_with_both_fee_sides(&pool).await;
    let repo = PgPoolAnalyticsRepository::new(pool);

    let buckets = repo
        .history(&addr.parse().unwrap(), 1)
        .await
        .expect("history should succeed");

    let bucket = buckets
        .iter()
        .find(|b| b.fees_usd.is_some())
        .expect("the seeded swaps must produce a valued bucket");

    assert_eq!(bucket.fees_usd, Some(expected_fees()));
    assert_eq!(bucket.protocol_fees_usd, Some(expected_protocol()));
    assert_eq!(bucket.referral_fees_usd, Some(expected_referral()));
    assert_eq!(
        bucket.lp_fees_usd,
        Some(expected_lp()),
        "the per-bucket split must match the windowed one — they read one view"
    );
}

#[sqlx::test]
async fn an_unvaluable_bucket_nulls_every_share(pool: PgPool) {
    // Mutation: the `valuation_complete` gate dropped from one share. ⚠️ Token B alone is priced
    // and carries the fee: with neither priced, `x * NULL` would hide a missing gate.
    let addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

    // Token A has no metadata row: the volume leg cannot be valued.
    insert_token(&pool, &mint_b, DEC_B, Some(PRICE_B)).await;
    insert_pool(&pool, &addr, &mint_a, &mint_b).await;

    insert_swap(
        &pool,
        Swap {
            addr: &addr,
            signature: "sig-unvaluable",
            direction: "a_to_b",
            amount_a: 1_000, // volume sits on the unvaluable side
            amount_b: 1_000_000,
            claiming: CLAIMING_B,
            protocol: PROTOCOL_B,
            compounding: COMPOUNDING_B,
            referral: REFERRAL_B,
            fee_token_is_a: false, // …while the fee sits on the priced one
            at: Utc::now() - Duration::hours(1),
        },
    )
    .await;

    let repo = PgPoolAnalyticsRepository::new(pool);
    let result = repo
        .batch_compute(&[addr.parse().unwrap()])
        .await
        .expect("batch_compute should succeed");
    let a = result.get(&addr.parse().unwrap()).unwrap();

    assert_eq!(
        (
            a.volume_24h_usd,
            a.fees_24h_usd,
            a.protocol_fees_24h_usd,
            a.referral_fees_24h_usd,
            a.lp_fees_24h_usd,
        ),
        (None, None, None, None, None),
        "a share surviving alone would make the split, effectiveFeeBps and the \
         coverage counters lie at once"
    );
    assert_eq!(
        a.swap_buckets_24h, 1,
        "the hour traded, and the coverage counters must still say so"
    );
    assert_eq!(a.swap_buckets_priced_24h, 0);
}
