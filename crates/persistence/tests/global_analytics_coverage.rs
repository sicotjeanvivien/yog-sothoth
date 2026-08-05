//! Integration test for the protocol-wide coverage counters behind
//! `GET /api/stats` (`PgGlobalAnalyticsRepository::global_analytics`).
//!
//! Gated behind `integration-tests`. Exists because the equivalent counters in
//! `pool_analytics` are covered and these were not: `rows_tests.rs` asserts the
//! row → domain mapping, which cannot catch the two `COUNT(*) FILTER` clauses
//! drifting from their per-pool twins. `/api/stats` is the most-read endpoint
//! of the dashboard; the number it prints deserves a test that can go red.

use super::helpers::pk;
use chrono::{Duration, Utc};
use sqlx::PgPool;

use yog_core::domain::{GlobalAnalyticsRepository, PoolAnalyticsRepository};
use yog_persistence::{PgGlobalAnalyticsRepository, PgPoolAnalyticsRepository};

/// One pool, its two mints registered with decimals, optionally priced.
async fn setup_pool(pool: &PgPool, seed: u8, price_b: Option<&str>) -> String {
    let pool_addr = pk(seed).to_string();
    let mint_a = format!("mint_a_{seed}");
    let mint_b = format!("mint_b_{seed}");
    let long_ago = Utc::now() - Duration::hours(48);

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
        .bind(long_ago)
        .execute(pool)
        .await
        .unwrap();
    }

    // Only ever token B: token A stays the unlisted side, so a priced pool is
    // valued through the implied rate — the shape this migration introduced.
    if let Some(price) = price_b {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
             VALUES ($1,$2::NUMERIC,'jupiter',$3)",
        )
        .bind(&mint_b)
        .bind(price)
        .bind(long_ago)
        .execute(pool)
        .await
        .unwrap();
    }

    pool_addr
}

/// Two swaps in one bucket: 1 000 A in against 0.5 B out, then 1.0 B in
/// against 2 000 A out — 3 000 A against 1.5 B over the hour.
async fn insert_bucket(pool: &PgPool, pool_addr: &str, tag: &str, hours_ago: i64) {
    let at = Utc::now() - Duration::hours(hours_ago);
    for (sig, dir, a, b, fee_is_a) in [
        (
            format!("{tag}_ab"),
            "a_to_b",
            1_000_000_000i64,
            500_000_000i64,
            true,
        ),
        (
            format!("{tag}_ba"),
            "b_to_a",
            2_000_000_000i64,
            1_000_000_000i64,
            false,
        ),
    ] {
        sqlx::query(
            "INSERT INTO meteora_damm_v2_swap_events
               (pool_address, signature, trade_direction,
                amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
                timestamp, slot, event_index)
             VALUES ($1,$2,$3,$4,$5,0,0,0,0,0,0,0,$6,$7,0,0)",
        )
        .bind(pool_addr)
        .bind(&sig)
        .bind(dir)
        .bind(a)
        .bind(b)
        .bind(fee_is_a)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
    }
}

#[sqlx::test]
async fn global_coverage_sums_every_pool_priced_or_not(pool: PgPool) {
    // Pool 1 is priceable and trades two hours; pool 2 has no price at all and
    // trades one. The global counters must report 2 valued buckets out of 3 —
    // the whole point being that the 112 (or whatever) USD figure next to them
    // covers two thirds of the activity, and says so.
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;

    insert_bucket(&pool, &priced, "p1", 1).await;
    insert_bucket(&pool, &priced, "p2", 3).await;
    insert_bucket(&pool, &unpriced, "u1", 2).await;

    let repo = PgGlobalAnalyticsRepository::new(pool.clone());
    let analytics = repo.global_analytics().await.unwrap();

    assert_eq!(
        analytics.swap_buckets_24h, 3,
        "three (pool, hour) buckets traded across the whole universe"
    );
    assert_eq!(
        analytics.swap_buckets_priced_24h, 2,
        "the unpriced pool's hour cannot be valued — reporting 3/3 here would \
         be the per-pool defect, escalated protocol-wide"
    );
    assert!(
        analytics.volume_24h_usd.is_some(),
        "the two valued buckets still produce a (partial) total"
    );
}

#[sqlx::test]
async fn global_coverage_ignores_activity_outside_the_window(pool: PgPool) {
    // The counters must obey the same 24h window as the sums they qualify;
    // a denominator on a wider window would understate coverage forever.
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    insert_bucket(&pool, &priced, "recent", 2).await;
    insert_bucket(&pool, &priced, "old", 30).await;

    let repo = PgGlobalAnalyticsRepository::new(pool.clone());
    let analytics = repo.global_analytics().await.unwrap();

    assert_eq!(analytics.swap_buckets_24h, 1, "the 30h-old bucket is out");
    assert_eq!(analytics.swap_buckets_priced_24h, 1);
}

#[sqlx::test]
async fn global_coverage_is_zero_zero_on_an_empty_universe(pool: PgPool) {
    // No pools, no swaps: COUNT never returns NULL, so the counters must be 0
    // and the USD sums None — "nothing happened", not "unknown coverage".
    let repo = PgGlobalAnalyticsRepository::new(pool.clone());
    let analytics = repo.global_analytics().await.unwrap();

    assert_eq!(analytics.swap_buckets_24h, 0);
    assert_eq!(analytics.swap_buckets_priced_24h, 0);
    assert_eq!(analytics.volume_24h_usd, None);
}

#[sqlx::test]
async fn the_global_counters_agree_with_the_per_pool_ones(pool: PgPool) {
    // The reason this file's header gives for existing — the two `COUNT(*)
    // FILTER` clauses drifting apart — was not actually asserted anywhere.
    // Here it is: same fixture, both read paths, one comparison.
    //
    // The relation is equality, not merely "close": every bucket the global
    // sees belongs to some pool, and both queries share the same window and
    // the same predicates.
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;

    insert_bucket(&pool, &priced, "p1", 1).await;
    insert_bucket(&pool, &priced, "p2", 3).await;
    insert_bucket(&pool, &unpriced, "u1", 2).await;

    let global = PgGlobalAnalyticsRepository::new(pool.clone())
        .global_analytics()
        .await
        .unwrap();

    let per_pool = PgPoolAnalyticsRepository::new(pool.clone())
        .batch_compute(&[pk(1), pk(2)])
        .await
        .unwrap();
    let summed_total: i64 = per_pool.values().map(|a| a.swap_buckets_24h).sum();
    let summed_priced: i64 = per_pool.values().map(|a| a.swap_buckets_priced_24h).sum();

    assert_eq!(
        global.swap_buckets_24h, summed_total,
        "the protocol-wide denominator must be the sum of the per-pool ones"
    );
    assert_eq!(
        global.swap_buckets_priced_24h, summed_priced,
        "…and so must the numerator, or /api/stats and /api/pools tell two \
         different stories about the same hours"
    );
    assert_eq!(
        (global.swap_buckets_24h, global.swap_buckets_priced_24h),
        (3, 2)
    );
}
