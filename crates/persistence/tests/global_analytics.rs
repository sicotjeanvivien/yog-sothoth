//! `PgGlobalAnalyticsRepository`, the read behind `GET /api/stats`: its
//! counters over the protocol aggregate, written here by `PgAnalyticRepository`
//! from swaps, and its TVL over the measurements as the writer leaves them.

use super::helpers::{pk, price_mint};
use chrono::{DateTime, Duration, DurationRound, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;

use yog_core::domain::{
    AnalyticRepository, CycleWrite, GlobalAnalytics, GlobalAnalyticsRepository,
    PoolAnalyticsRepository,
};
use yog_persistence::{
    PgAnalyticRepository, PgGlobalAnalyticsRepository, PgPoolAnalyticsRepository,
};

fn hour_of(at: DateTime<Utc>) -> DateTime<Utc> {
    at.duration_trunc(Duration::hours(1)).unwrap()
}

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
    //
    // Sampled hourly rather than once at `long_ago`: since migration 005 the
    // as-of lookup reaches back at most `yog_price_max_age_asof()` from the
    // bucket's start, so a single 48-hour-old row prices no bucket at all.
    if let Some(price) = price_b {
        price_mint(pool, &mint_b, price).await;
    }

    pool_addr
}

/// Two swaps in one bucket: 1 000 A in against 0.5 B out, then 1.0 B in
/// against 2 000 A out — 3 000 A against 1.5 B over the hour.
///
/// ⚠️ Both legs follow `collect_fee_mode = 0` (BothToken): the fee is charged on
/// the OUT token, so `a_to_b` pays in B and `b_to_a` pays in A, and neither fee
/// is zero. Neither detail is decoration — `a_to_b` with the fee on A exists in
/// no mode (0 of 662 real swaps) and no real swap carries a zero fee, and
/// `valuation_complete` reads `fee_in_a` / `fee_in_b`. A fixture that cannot
/// occur is a test that cannot fail for the right reason.
async fn insert_bucket(pool: &PgPool, pool_addr: &str, tag: &str, hours_ago: i64) {
    let at = Utc::now() - Duration::hours(hours_ago);
    for (sig, dir, a, b, fee, fee_is_a) in [
        (
            format!("{tag}_ab"),
            "a_to_b",
            1_000_000_000i64,
            500_000_000i64,
            5_000_000i64, // 0.005 B — the out token
            false,
        ),
        (
            format!("{tag}_ba"),
            "b_to_a",
            2_000_000_000i64,
            1_000_000_000i64,
            100_000_000i64, // 100 A — the out token
            true,
        ),
    ] {
        sqlx::query(
            "INSERT INTO meteora_damm_v2_swap_events
               (pool_address, signature, trade_direction,
                amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
                timestamp, slot, event_index)
             VALUES ($1,$2,$3,$4,$5,0,0,0,$6,0,0,0,$7,$8,0,0)",
        )
        .bind(pool_addr)
        .bind(&sig)
        .bind(dir)
        .bind(a)
        .bind(b)
        .bind(fee)
        .bind(fee_is_a)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
    }
}

/// The facts and the aggregate of every hour a fixture trades in, open as the
/// writer leaves the current one.
async fn write_facts(pool: &PgPool) {
    let now = Utc::now();
    let cycle = CycleWrite {
        from: hour_of(now - Duration::hours(32)),
        through: hour_of(now),
        freeze_through: None,
        tvl_hour: None,
        at: now,
    };
    PgAnalyticRepository::new(pool.clone())
        .write_cycle(&cycle)
        .await
        .unwrap();
}

/// A TVL measurement of the hour `hours_ago` back.
async fn measure_tvl(pool: &PgPool, protocol: &str, hours_ago: i64, tvl_usd: i64, priced: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl (protocol, bucket, tvl_usd, pools_priced, measured_at)
         VALUES ($1, $2, $3, $4, $2)",
    )
    .bind(protocol)
    .bind(hour_of(Utc::now() - Duration::hours(hours_ago)))
    .bind(tvl_usd)
    .bind(priced)
    .execute(pool)
    .await
    .unwrap();
}

async fn read(pool: &PgPool) -> GlobalAnalytics {
    PgGlobalAnalyticsRepository::new(pool.clone())
        .global_analytics()
        .await
        .unwrap()
}

/// Against the valued count read off the wrong column: the unpriced pool's
/// hour counted as valued, 3 of 3.
#[sqlx::test]
async fn global_coverage_sums_every_pool_priced_or_not(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;

    insert_bucket(&pool, &priced, "p1", 1).await;
    insert_bucket(&pool, &priced, "p2", 3).await;
    insert_bucket(&pool, &unpriced, "u1", 2).await;
    write_facts(&pool).await;

    let analytics = read(&pool).await;

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

/// Against a window wider than 24 h: the 30 h-old hour, written, counted.
#[sqlx::test]
async fn global_coverage_ignores_activity_outside_the_window(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    insert_bucket(&pool, &priced, "recent", 2).await;
    insert_bucket(&pool, &priced, "old", 30).await;
    write_facts(&pool).await;

    let hours: i64 = sqlx::query_scalar("SELECT count(*) FROM analytic_protocol_hourly")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hours, 2, "both hours are in the aggregate");

    let analytics = read(&pool).await;

    assert_eq!(analytics.swap_buckets_24h, 1, "the 30h-old bucket is out");
    assert_eq!(analytics.swap_buckets_priced_24h, 1);
}

/// Against a `COALESCE` dropped: an empty window sums to NULL, which the row
/// refuses for a count.
#[sqlx::test]
async fn global_coverage_is_zero_zero_on_an_empty_universe(pool: PgPool) {
    let analytics = read(&pool).await;

    assert_eq!(analytics.swap_buckets_24h, 0);
    assert_eq!(analytics.swap_buckets_priced_24h, 0);
    assert_eq!(analytics.volume_24h_usd, None);
    assert_eq!((analytics.total_tvl_usd, analytics.pools_priced), (None, 0));
}

/// Against the aggregate and the activity view, read per pool, counting
/// different buckets of the same hours.
#[sqlx::test]
async fn the_global_counters_agree_with_the_per_pool_ones(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;

    insert_bucket(&pool, &priced, "p1", 1).await;
    insert_bucket(&pool, &priced, "p2", 3).await;
    insert_bucket(&pool, &unpriced, "u1", 2).await;
    write_facts(&pool).await;

    let global = read(&pool).await;

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

/// Against every measurement summed (160 over 6 pools) or a single protocol
/// read (100 over 3).
#[sqlx::test]
async fn the_tvl_is_the_last_measurement_of_each_protocol_summed(pool: PgPool) {
    measure_tvl(&pool, "meteora_damm_v2", 1, 10, 1).await;
    measure_tvl(&pool, "meteora_damm_v2", 0, 100, 3).await;
    measure_tvl(&pool, "meteora_dlmm", 0, 50, 2).await;

    let analytics = read(&pool).await;

    assert_eq!(
        (analytics.total_tvl_usd, analytics.pools_priced),
        (Some(Decimal::from(150)), 5)
    );
}

/// Against the age limit dropped (the measurement two hours back read) or
/// narrowed to the current hour (the previous hour's not read).
#[sqlx::test]
async fn a_tvl_measured_before_the_previous_hour_reads_as_null(pool: PgPool) {
    measure_tvl(&pool, "meteora_damm_v2", 2, 100, 3).await;
    let stale = read(&pool).await;
    assert_eq!((stale.total_tvl_usd, stale.pools_priced), (None, 0));

    measure_tvl(&pool, "meteora_damm_v2", 1, 80, 2).await;
    let previous_hour = read(&pool).await;
    assert_eq!(
        (previous_hour.total_tvl_usd, previous_hour.pools_priced),
        (Some(Decimal::from(80)), 2)
    );
}
