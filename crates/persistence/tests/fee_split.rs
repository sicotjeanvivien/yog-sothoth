//! Integration test for the realized-fee split published by
//! `meteora_damm_v2_pool_hourly_activity` (migration 007) and read back through
//! `PgPoolAnalyticsRepository::batch_compute` / `history`.
//!
//! Gated behind `integration-tests`. The finding it guards (`.project` ticket
//! 05): the LP share was published as `fees - protocol_fees`, which credits the
//! referral to the liquidity providers. cp-amm takes the referral out of the
//! PROTOCOL share (`cp-amm/src/state/fee.rs::split_fees`), so the LP share is
//! `claiming + compounding`.
//!
//! ## What makes this fixture bite, component by component
//!
//! Every number below is chosen to defeat one specific wrong implementation
//! that would otherwise produce the right answer. A first version of this file
//! got two of them wrong and the code review caught it, so they are spelled out:
//!
//!   * **The four components are pairwise distinct on each side.** With
//!     `referral = 0` the two formulas agree and the test asserts nothing. With
//!     `referral = compounding` — the actual bug in the first version —
//!     `SUM(compounding_fee)` mis-typed into the `referral_fee_in_*` columns
//!     reads correct. With `protocol = referral` a doubled subtraction lands on
//!     the right number.
//!   * **The fee is charged in token A on one swap and in token B on the
//!     other**, and the two tokens have different decimals AND different
//!     prices. The two cagg columns are `FILTER (WHERE fee_token_is_a)` /
//!     `FILTER (WHERE NOT ...)`, and with a single fee side — or with two
//!     identically-scaled tokens — swapping those two filters is invisible.
//!     Here it turns $5.15 of referral into ~$50 000.
//!   * **The unvaluable case is asymmetric** (token A unresolved, token B
//!     priced and carrying the fee), because a bucket where *neither* side is
//!     priced NULLs every share through plain arithmetic — it cannot tell
//!     whether the `valuation_complete` gate is still there. See
//!     `an_unvaluable_bucket_nulls_every_share`.

use super::helpers::{pk, price_mint};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::PoolAnalyticsRepository;
use yog_persistence::PgPoolAnalyticsRepository;

// ── Token scales and prices, deliberately different on the two sides ─────────
const DEC_A: i32 = 0;
const DEC_B: i32 = 6;
const PRICE_A: &str = "1.0";
const PRICE_B: &str = "3.0";

// ── Swap 1: fee charged in token A (raw = USD, since A is $1 with 0 decimals)
const CLAIMING_A: i64 = 70;
const PROTOCOL_A: i64 = 21;
const COMPOUNDING_A: i64 = 4;
const REFERRAL_A: i64 = 5;

// ── Swap 2: fee charged in token B (1 000 000 raw = 1.0 B = $3)
const CLAIMING_B: i64 = 700_000;
const PROTOCOL_B: i64 = 210_000;
const COMPOUNDING_B: i64 = 40_000;
const REFERRAL_B: i64 = 50_000;

/// Expected USD figures, written as literals rather than recomputed from the
/// constants: a test that re-derives the value with the same formula as the
/// code under test proves only that the formula is stable.
///
/// ```text
/// fees     = 100 (A)          + 3.00 (B: 1 000 000 / 1e6 × $3)  = 103.00
/// protocol =  21              + 0.63 (210 000 / 1e6 × $3)       =  21.63
/// referral =   5              + 0.15 ( 50 000 / 1e6 × $3)       =   5.15
/// lp       =  74 (70 + 4)     + 2.22 (740 000 / 1e6 × $3)       =  76.22
/// ```
///
/// The wrong formula (`fees − protocol`) gives **81.37**, not 76.22.
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
/// A params struct rather than a long argument list — same shape as
/// `implied_price_coverage.rs`'s `Swap`.
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

    // An hour ago: inside the 24h window, on a bucket the hourly price series
    // covers (see `price_mint`'s doc — one row prices exactly one bucket).
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

    // The property the three shares must have, stated as itself rather than
    // left implied by the three values above.
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

    // Both swaps share a bucket, so exactly one carries fees.
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
    // `valuation_complete` governs all five figures at once, so no share may be
    // a number while the total is unknown.
    //
    // ⚠️ The asymmetry is the whole test. Price NEITHER token and every share
    // NULLs through plain arithmetic (`x * NULL`), so the gate could be gone and
    // nothing would show. Here token B is priced and carries the entire fee: its
    // shares are perfectly computable on their own, and only the gate stops
    // them. Removing the gate from `referral_fees_usd` alone surfaces $0.15
    // next to a NULL total — the shape this migration says is unrepresentable.
    let addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

    // Token A: no metadata row at all — `POWER(10, NULL)` also kills the implied
    // rate, so the volume leg is unvaluable and cannot be rescued.
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
