//! The rebuild of 031: every table valued at the pool's mid price emptied, and the writer's
//! floor kept, read on the 5-minute aggregate rather than on the raw swaps.

use chrono::{DateTime, Duration, Utc};
use sqlx::migrate::Migrator;
use sqlx::{AssertSqlSafe, PgPool};
use yog_core::domain::{AnalyticRepository, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::as_role;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// The day `$1` days back on the database's clock, at midnight UTC.
macro_rules! day {
    () => {
        "date_trunc('day', now(), 'UTC') - $1 * INTERVAL '1 day'"
    };
}

/// One swap `days` back at 01:00, materialised into the 5-minute aggregate.
async fn swap_materialised(pool: &PgPool, days: i32) {
    sqlx::query(concat!(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ('p', 's' || $1, 'a_to_b', 1, 1, 0, 0, 1, 0, 0, 0, 0, false, ",
        day!(),
        " + INTERVAL '1 hour', 0, 0)"
    ))
    .bind(days)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', NULL, NULL)",
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Three frozen hours of the writer, from midnight `days` back.
async fn frozen_hours_from(pool: &PgPool, days: i32) {
    sqlx::query(concat!(
        "INSERT INTO analytic_hours (bucket, frozen_at) SELECT ",
        day!(),
        " + h * INTERVAL '1 hour', now() FROM generate_series(0, 2) h"
    ))
    .bind(days)
    .execute(pool)
    .await
    .unwrap();
}

/// `(bucket, frozen)` of every hour left.
async fn hours(pool: &PgPool) -> Vec<(DateTime<Utc>, bool)> {
    sqlx::query_as("SELECT bucket, frozen_at IS NOT NULL FROM analytic_hours ORDER BY bucket")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn midnight(pool: &PgPool, days: i32) -> DateTime<Utc> {
    sqlx::query_scalar(concat!("SELECT ", day!()))
        .bind(days)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Against a table left out of the `TRUNCATE`: its rows, valued at the pool's mid price,
/// would stay published beside the ones written again.
#[sqlx::test(migrations = false)]
async fn the_rebuild_empties_the_valued_tables(pool: PgPool) {
    MIGRATOR.run_to(30, &pool).await.unwrap();
    swap_materialised(&pool, 5).await;
    frozen_hours_from(&pool, 3).await;
    for insert in [
        "INSERT INTO analytic_tranches (bucket, written_at) VALUES (now(), now())",
        "INSERT INTO analytic_pool_prices_5min (pool_address, bucket) VALUES ('p', now())",
        "INSERT INTO analytic_pool_5min (pool_address, bucket, protocol, swap_count, valued)
         VALUES ('p', now(), 'meteora_damm_v2', 1, false)",
        "INSERT INTO analytic_pool_hourly (pool_address, bucket, protocol, swap_count, valued)
         VALUES ('p', now(), 'meteora_damm_v2', 1, false)",
        "INSERT INTO analytic_protocol_hourly
           (protocol, bucket, swap_count, active_pools, active_pools_valued)
         VALUES ('meteora_damm_v2', now(), 1, 1, 0)",
        "INSERT INTO analytic_token_periods
           (period, mint, bucket, volume_usd, pools, pool_hours, pool_hours_valued)
         VALUES ('24h', 'm', now(), 801792609, 1, 1, 1)",
        "INSERT INTO analytic_protocol_periods (protocol, period, bucket, active_pools)
         VALUES ('meteora_damm_v2', '24h', now(), 1)",
    ] {
        sqlx::query(insert).execute(&pool).await.unwrap();
    }

    MIGRATOR.run_to(31, &pool).await.unwrap();

    for table in [
        "analytic_tranches",
        "analytic_pool_prices_5min",
        "analytic_pool_5min",
        "analytic_pool_hourly",
        "analytic_protocol_hourly",
        "analytic_token_periods",
        "analytic_protocol_periods",
    ] {
        let rows: i64 = sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "{table} must be emptied");
    }
    assert_eq!(
        hours(&pool).await,
        [(midnight(&pool, 3).await, false)],
        "the floor alone stays, unfrozen"
    );
}

/// Against the floor read on the raw swaps: their retention gone past it, the writer would
/// lose every hour the aggregate still covers.
#[sqlx::test(migrations = false)]
async fn the_floor_stays_when_the_raw_is_gone(pool: PgPool) {
    MIGRATOR.run_to(30, &pool).await.unwrap();
    swap_materialised(&pool, 5).await;
    frozen_hours_from(&pool, 3).await;
    sqlx::query("DELETE FROM meteora_damm_v2_swap_events")
        .execute(&pool)
        .await
        .unwrap();

    MIGRATOR.run_to(31, &pool).await.unwrap();

    assert_eq!(hours(&pool).await, [(midnight(&pool, 3).await, false)]);
}

/// Against the writer's first hour taken alone: older than what the aggregate covers, its
/// hours would be written again with no swap, and published as empty; and against a whole
/// day skipped, which an aggregate begun today would push to tomorrow.
#[sqlx::test(migrations = false)]
async fn the_floor_waits_for_the_aggregate(pool: PgPool) {
    MIGRATOR.run_to(30, &pool).await.unwrap();
    swap_materialised(&pool, 3).await;
    frozen_hours_from(&pool, 5).await;

    MIGRATOR.run_to(31, &pool).await.unwrap();

    assert_eq!(
        hours(&pool).await,
        [(midnight(&pool, 3).await + Duration::hours(2), false)],
        "the hour after the aggregate's first, at 01:00"
    );
}

/// Against an aggregate read as no bound when it is empty: the writer's old hours would be
/// written again with no swap, and published as empty.
#[sqlx::test(migrations = false)]
async fn the_floor_needs_the_aggregate(pool: PgPool) {
    MIGRATOR.run_to(30, &pool).await.unwrap();
    frozen_hours_from(&pool, 3).await;

    MIGRATOR.run_to(31, &pool).await.unwrap();

    assert_eq!(hours(&pool).await, []);
}

/// Against the periods kept: written once at each freeze, the first hour frozen again past
/// one of theirs would fail its cycle, and every cycle after it.
#[sqlx::test(migrations = false)]
async fn the_writer_freezes_again_after_the_rebuild(pool: PgPool) {
    MIGRATOR.run_to(30, &pool).await.unwrap();
    swap_materialised(&pool, 5).await;
    frozen_hours_from(&pool, 3).await;
    sqlx::query(concat!(
        "INSERT INTO analytic_protocol_periods (protocol, period, bucket, active_pools)
         SELECT 'meteora_damm_v2', w, ",
        day!(),
        " + h * INTERVAL '1 hour', 1
           FROM generate_series(0, 23) h, unnest(ARRAY['24h', '7d', '30d']) w"
    ))
    .bind(3)
    .execute(&pool)
    .await
    .unwrap();

    // ⚠️ Every migration before the writer, whose queries follow the latest schema.
    MIGRATOR.run(&pool).await.unwrap();

    let analytic = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    let progress = analytic.progress().await.unwrap();
    let written = analytic
        .write_cycle(&CycleWrite::plan(Utc::now(), progress))
        .await
        .expect("the hours frozen again find no period of before");
    assert!(written.hours_frozen > 0);
    let periods: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM analytic_protocol_periods WHERE protocol = 'meteora_damm_v2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        periods as u64,
        written.hours_frozen * 3,
        "one row per hour frozen and window"
    );
}
