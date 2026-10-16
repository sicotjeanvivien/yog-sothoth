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
    let mint_a = pk(seed + 100).to_string();
    let mint_b = pk(seed + 200).to_string();
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

    // Only token B: a priced pool is valued through the implied rate. ⚠️ Hourly: one row
    // prices one bucket.
    if let Some(price) = price_b {
        price_mint(pool, &mint_b, price).await;
    }

    pool_addr
}

/// Two swaps in one bucket, 3 000 A against 1.5 B. ⚠️ Each fee on the OUT token and non-zero:
/// `valuation_complete` reads the fee sides, and an impossible fixture fails for the wrong reason.
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
             VALUES ($1,$2,$3,$4,$5,0,0,0,$6,$7,0,0,$8,$9,0,0)",
        )
        .bind(pool_addr)
        .bind(&sig)
        .bind(dir)
        .bind(a)
        .bind(b)
        .bind(fee)
        .bind(fee / 4)
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
    write_facts_spanning(pool, 32, 0).await;
}

/// As [`write_facts`], by a writer whose first and last hours written are
/// `first` and `last` hours back on the database's clock.
async fn write_facts_spanning(pool: &PgPool, first: i64, last: i64) {
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT now()")
        .fetch_one(pool)
        .await
        .unwrap();
    let cycle = CycleWrite {
        from: hour_of(now - Duration::hours(first)),
        through: hour_of(now - Duration::hours(last)),
        freeze_through: None,
        tvl_hour: None,
        at: now,
    };
    PgAnalyticRepository::new(pool.clone())
        .write_cycle(&cycle)
        .await
        .unwrap();
}

/// A TVL measurement of the hour `hours_ago` back, by the database's clock,
/// which is the one the read compares it to.
async fn measure_tvl(pool: &PgPool, protocol: &str, hours_ago: i32, tvl_usd: i64, priced: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl (protocol, bucket, tvl_usd, pools_priced, measured_at)
         VALUES ($1, date_trunc('hour', now(), 'UTC') - make_interval(hours => $2), $3, $4, now())",
    )
    .bind(protocol)
    .bind(hours_ago)
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

/// Against a sum read off the wrong column: the LP or protocol share for the
/// fees, any other for the volume.
#[sqlx::test]
async fn the_usd_sums_equal_the_activity_view(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    insert_bucket(&pool, &priced, "p1", 1).await;
    insert_bucket(&pool, &priced, "p2", 3).await;
    write_facts(&pool).await;

    let view: (Option<Decimal>, Option<Decimal>) = sqlx::query_as(
        "SELECT SUM(volume_usd), SUM(fees_usd) FROM meteora_damm_v2_pool_hourly_activity
          WHERE bucket > now() - INTERVAL '24 hours'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(view.0.is_some() && view.1.is_some(), "{view:?}");

    let analytics = read(&pool).await;

    assert_eq!((analytics.volume_24h_usd, analytics.fees_24h_usd), view);
}

/// Against the writer's age dropped (the hours of a writer stopped two hours
/// back read) or narrowed to the current hour (the previous hour's not read).
#[sqlx::test]
async fn a_writer_behind_the_previous_hour_reads_as_no_volume(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    insert_bucket(&pool, &priced, "p3", 3).await;

    write_facts_spanning(&pool, 32, 2).await;
    let facts: i64 = sqlx::query_scalar("SELECT count(*) FROM analytic_protocol_hourly")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(facts, 1, "the hour traded is in the aggregate");
    let stopped = read(&pool).await;
    assert_eq!((stopped.volume_24h_usd, stopped.fees_24h_usd), (None, None));
    assert_eq!(
        (stopped.swap_buckets_24h, stopped.swap_buckets_priced_24h),
        (0, 0)
    );

    write_facts_spanning(&pool, 32, 1).await;
    let previous_hour = read(&pool).await;
    assert!(previous_hour.volume_24h_usd.is_some());
    assert_eq!(
        (
            previous_hour.swap_buckets_24h,
            previous_hour.swap_buckets_priced_24h
        ),
        (1, 1)
    );
}

/// Against the writer's first hour not checked (the 10 hours of a new writer
/// read) or required an hour early (the window's first hour not enough).
#[sqlx::test]
async fn a_writer_younger_than_the_window_reads_as_no_volume(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    insert_bucket(&pool, &priced, "p3", 3).await;

    write_facts_spanning(&pool, 10, 0).await;
    let facts: i64 = sqlx::query_scalar("SELECT count(*) FROM analytic_protocol_hourly")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(facts, 1, "the hour traded is in the aggregate");
    let young = read(&pool).await;
    assert_eq!((young.volume_24h_usd, young.fees_24h_usd), (None, None));
    assert_eq!(
        (young.swap_buckets_24h, young.swap_buckets_priced_24h),
        (0, 0)
    );

    write_facts_spanning(&pool, 23, 0).await;
    let whole_window = read(&pool).await;
    assert!(whole_window.volume_24h_usd.is_some());
    assert_eq!(
        (
            whole_window.swap_buckets_24h,
            whole_window.swap_buckets_priced_24h
        ),
        (1, 1)
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
