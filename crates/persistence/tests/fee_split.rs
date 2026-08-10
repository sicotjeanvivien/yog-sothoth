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
//! ## What makes this test bite
//!
//! The fixture charges **four distinct, non-zero components**. That is not
//! decoration:
//!
//!   * with `referral_fee = 0` the two formulas agree and the test would pass
//!     either way — it would assert nothing;
//!   * with `protocol_fee = referral_fee` a doubled subtraction would still
//!     land on the right number;
//!   * with `claiming = compounding` a share swapped for the other would go
//!     unnoticed.
//!
//! Both mints are priced at $1 with 0 decimals, so a USD figure equals its raw
//! lamport count and the arithmetic is legible in the assertions.

use super::helpers::{pk, price_mint};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::PoolAnalyticsRepository;
use yog_persistence::PgPoolAnalyticsRepository;

/// The four components of one swap's fee, charged in token A.
///
/// `70 + 20 + 5 + 5 = 100` total, of which the LP share is `claiming +
/// compounding = 75`. The wrong formula (`fees - protocol`) yields 80.
const CLAIMING: i64 = 70;
const PROTOCOL: i64 = 20;
const COMPOUNDING: i64 = 5;
const REFERRAL: i64 = 5;

const FEE_TOTAL: i64 = CLAIMING + PROTOCOL + COMPOUNDING + REFERRAL;
const LP_SHARE: i64 = CLAIMING + COMPOUNDING;

async fn seed_pool_with_one_swap(pool: &PgPool) -> (String, String, String) {
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

    for mint in [&mint_a, &mint_b] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, 0, NOW(), NOW())",
        )
        .bind(mint)
        .execute(pool)
        .await
        .unwrap();
        price_mint(pool, mint, "1.0").await;
    }

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

    // One swap, an hour ago so it sits inside the 24h window on a bucket whose
    // price observations exist.
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1,'sig-fee-split','a_to_b',1000,1000,0,0,0,$2,$3,$4,$5,true,$6,0,0)",
    )
    .bind(&pool_addr)
    .bind(CLAIMING)
    .bind(PROTOCOL)
    .bind(COMPOUNDING)
    .bind(REFERRAL)
    .bind(Utc::now() - Duration::hours(1))
    .execute(pool)
    .await
    .unwrap();

    (pool_addr, mint_a, mint_b)
}

fn usd(raw: i64) -> Decimal {
    Decimal::new(raw, 0)
}

#[sqlx::test]
async fn batch_compute_excludes_the_referral_from_the_lp_share(pool: PgPool) {
    let (pool_addr, _, _) = seed_pool_with_one_swap(&pool).await;
    let repo = PgPoolAnalyticsRepository::new(pool);

    let result = repo
        .batch_compute(&[pool_addr.parse().unwrap()])
        .await
        .expect("batch_compute should succeed");
    let analytics = result
        .get(&pool_addr.parse().unwrap())
        .expect("the seeded pool must be in the batch result");

    assert_eq!(
        analytics.fees_24h_usd,
        Some(usd(FEE_TOTAL)),
        "the total is the sum of the four components — unchanged by this fix"
    );
    assert_eq!(analytics.protocol_fees_24h_usd, Some(usd(PROTOCOL)));
    assert_eq!(analytics.referral_fees_24h_usd, Some(usd(REFERRAL)));
    assert_eq!(
        analytics.lp_fees_24h_usd,
        Some(usd(LP_SHARE)),
        "the LP share is claiming + compounding ({LP_SHARE}); \
         `fees - protocol` would give {}",
        FEE_TOTAL - PROTOCOL
    );

    // The property the three shares must have, stated as itself rather than
    // implied by the three values above.
    assert_eq!(
        analytics.lp_fees_24h_usd.unwrap()
            + analytics.protocol_fees_24h_usd.unwrap()
            + analytics.referral_fees_24h_usd.unwrap(),
        analytics.fees_24h_usd.unwrap(),
        "the three shares must partition the total exactly"
    );
}

#[sqlx::test]
async fn history_publishes_the_same_split_as_the_24h_analytics(pool: PgPool) {
    let (pool_addr, _, _) = seed_pool_with_one_swap(&pool).await;
    let repo = PgPoolAnalyticsRepository::new(pool);

    let buckets = repo
        .history(&pool_addr.parse().unwrap(), 1)
        .await
        .expect("history should succeed");

    // One swap, so exactly one bucket carries fees.
    let bucket = buckets
        .iter()
        .find(|b| b.fees_usd.is_some())
        .expect("the seeded swap must produce a valued bucket");

    assert_eq!(bucket.fees_usd, Some(usd(FEE_TOTAL)));
    assert_eq!(bucket.protocol_fees_usd, Some(usd(PROTOCOL)));
    assert_eq!(bucket.referral_fees_usd, Some(usd(REFERRAL)));
    assert_eq!(
        bucket.lp_fees_usd,
        Some(usd(LP_SHARE)),
        "the per-bucket split must match the windowed one — they read one view"
    );
}

#[sqlx::test]
async fn an_unpriceable_bucket_nulls_the_shares_together(pool: PgPool) {
    // `valuation_complete` governs all four figures at once, so a share can
    // never be a number while the total is unknown — the shape that would let a
    // consumer read a partial split as a whole one.
    let pool_addr = pk(1).to_string();
    let mint_a = pk(2).to_string();
    let mint_b = pk(3).to_string();

    // Metadata for both, price for neither: nothing can be valued.
    for mint in [&mint_a, &mint_b] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, 0, NOW(), NOW())",
        )
        .bind(mint)
        .execute(&pool)
        .await
        .unwrap();
    }

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

    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1,'sig-unpriced','a_to_b',1000,1000,0,0,0,$2,$3,$4,$5,true,$6,0,0)",
    )
    .bind(&pool_addr)
    .bind(CLAIMING)
    .bind(PROTOCOL)
    .bind(COMPOUNDING)
    .bind(REFERRAL)
    .bind(Utc::now() - Duration::hours(1))
    .execute(&pool)
    .await
    .unwrap();

    let repo = PgPoolAnalyticsRepository::new(pool);
    let result = repo
        .batch_compute(&[pool_addr.parse().unwrap()])
        .await
        .expect("batch_compute should succeed");
    let analytics = result.get(&pool_addr.parse().unwrap()).unwrap();

    assert_eq!(analytics.fees_24h_usd, None);
    assert_eq!(analytics.protocol_fees_24h_usd, None);
    assert_eq!(analytics.referral_fees_24h_usd, None);
    assert_eq!(
        analytics.lp_fees_24h_usd, None,
        "the LP share is a subtraction of unknowns — it must not surface as 0"
    );
    assert_eq!(
        analytics.swap_buckets_24h, 1,
        "the hour traded, and the coverage counters must still say so"
    );
    assert_eq!(analytics.swap_buckets_priced_24h, 0);
}
