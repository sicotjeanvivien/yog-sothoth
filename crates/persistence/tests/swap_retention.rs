//! The raw swaps are chunked by the day, compressed a day after their chunk closes, and kept
//! 7 days — read from the TimescaleDB catalog, in seconds (a job's text follows `IntervalStyle`).
//! ⚠️ The policies never fire here: this is what is declared, not what a scheduler does.

use sqlx::{PgPool, Row};
use yog_core::domain::FREEZE_MARGIN;

const HOUR: i64 = 3_600;
const DAY: i64 = 24 * HOUR;

const SWAPS: &str = "meteora_damm_v2_swap_events";
const SWAPS_HOURLY: &str = "meteora_damm_v2_swap_events_hourly";
const SWAPS_5MIN: &str = "meteora_damm_v2_swap_events_5min";

/// The storage settings of the raw swaps, in seconds. `None` is a policy that
/// does not exist.
#[derive(Debug, PartialEq, Eq)]
struct SwapStorage {
    chunk: Option<i64>,
    compress_after: Option<i64>,
    drop_after: Option<i64>,
    refresh_start_offset: Option<i64>,
    refresh_end_offset: Option<i64>,
    refresh_schedule: Option<i64>,
}

/// Mutation this is written against: any one of the four settings of 014
/// changed, or its policy removed.
#[sqlx::test]
async fn the_raw_swaps_take_the_four_settings(pool: PgPool) {
    let row = sqlx::query(
        "WITH rf AS (
             SELECT config, schedule_interval
               FROM timescaledb_information.jobs
              WHERE proc_name = 'policy_refresh_continuous_aggregate'
                AND hypertable_name = $2
         )
         SELECT
           (SELECT extract(epoch FROM time_interval)::BIGINT
              FROM timescaledb_information.dimensions
             WHERE hypertable_name = $1 AND dimension_number = 1)        AS chunk,
           (SELECT extract(epoch FROM (config->>'compress_after')::interval)::BIGINT
              FROM timescaledb_information.jobs
             WHERE proc_name = 'policy_compression'
               AND hypertable_name = $1)                                 AS compress_after,
           (SELECT extract(epoch FROM (config->>'drop_after')::interval)::BIGINT
              FROM timescaledb_information.jobs
             WHERE proc_name = 'policy_retention'
               AND hypertable_name = $1)                                 AS drop_after,
           (SELECT extract(epoch FROM (config->>'start_offset')::interval)::BIGINT
              FROM rf)                                                   AS refresh_start_offset,
           (SELECT extract(epoch FROM (config->>'end_offset')::interval)::BIGINT
              FROM rf)                                                   AS refresh_end_offset,
           (SELECT extract(epoch FROM schedule_interval)::BIGINT FROM rf) AS refresh_schedule",
    )
    .bind(SWAPS)
    .bind(SWAPS_HOURLY)
    .fetch_one(&pool)
    .await
    .expect("the swap settings must be readable, one policy of each kind");

    let declared = SwapStorage {
        chunk: row.get("chunk"),
        compress_after: row.get("compress_after"),
        drop_after: row.get("drop_after"),
        refresh_start_offset: row.get("refresh_start_offset"),
        refresh_end_offset: row.get("refresh_end_offset"),
        refresh_schedule: row.get("refresh_schedule"),
    };

    assert_eq!(
        declared,
        SwapStorage {
            chunk: Some(DAY),
            compress_after: Some(DAY),
            drop_after: Some(7 * DAY),
            refresh_start_offset: Some(6 * DAY),
            refresh_end_offset: Some(HOUR),
            refresh_schedule: Some(HOUR),
        }
    );
}

/// The tranches valued by yog-analytic (028), kept as long as the raw swaps.
const TRANCHES: [&str; 2] = ["analytic_pool_prices_5min", "analytic_pool_5min"];

/// Mutation this is written against: one line of 014 pointed at another
/// table, or the swap settings made the house default.
#[sqlx::test]
async fn no_other_hypertable_moves(pool: PgPool) {
    let chunks = settings(
        &pool,
        "SELECT hypertable_name::TEXT, extract(epoch FROM time_interval)::BIGINT
           FROM timescaledb_information.dimensions
          WHERE hypertable_schema = 'public' AND dimension_number = 1
            AND hypertable_name <> $1",
        SWAPS,
    )
    .await;
    assert_house("chunk_time_interval", &chunks, |table| {
        if TRANCHES.contains(&table) {
            DAY
        } else {
            7 * DAY
        }
    });

    let compression = settings(
        &pool,
        "SELECT hypertable_name::TEXT,
                extract(epoch FROM (config->>'compress_after')::interval)::BIGINT
           FROM timescaledb_information.jobs
          WHERE proc_name = 'policy_compression' AND hypertable_name <> $1",
        SWAPS,
    )
    .await;
    assert_house("compress_after", &compression, |table| {
        if table == "signals" {
            30 * DAY
        } else {
            7 * DAY
        }
    });

    let retention = settings(
        &pool,
        "SELECT hypertable_name::TEXT,
                extract(epoch FROM (config->>'drop_after')::interval)::BIGINT
           FROM timescaledb_information.jobs
          WHERE proc_name = 'policy_retention' AND hypertable_name <> $1",
        SWAPS,
    )
    .await;
    assert_house("drop_after", &retention, |table| {
        if TRANCHES.contains(&table) {
            7 * DAY
        } else {
            30 * DAY
        }
    });

    let refresh = settings(
        &pool,
        "SELECT hypertable_name::TEXT,
                extract(epoch FROM (config->>'start_offset')::interval)::BIGINT
           FROM timescaledb_information.jobs
          WHERE proc_name = 'policy_refresh_continuous_aggregate'
            AND hypertable_name <> $1",
        SWAPS_HOURLY,
    )
    .await;
    assert_house("start_offset", &refresh, |table| {
        if table == SWAPS_5MIN {
            6 * DAY
        } else {
            29 * DAY
        }
    });
}

/// `start_offset`, `end_offset`, schedule, newest batch first, `drop_after` — seconds.
type TrancheSettings = (
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<bool>,
    Option<i64>,
);

/// Mutation this is written against: any setting of 021 changed, its policy removed, or the
/// freeze margin moved without its `end_offset`.
#[sqlx::test]
async fn the_5_minute_tranches_take_their_settings(pool: PgPool) {
    let row: TrancheSettings = sqlx::query_as(
        "WITH rf AS (
                 SELECT config, schedule_interval
                   FROM timescaledb_information.jobs
                  WHERE proc_name = 'policy_refresh_continuous_aggregate'
                    AND hypertable_name = $1
             )
             SELECT
               (SELECT extract(epoch FROM (config->>'start_offset')::interval)::BIGINT FROM rf),
               (SELECT extract(epoch FROM (config->>'end_offset')::interval)::BIGINT FROM rf),
               (SELECT extract(epoch FROM schedule_interval)::BIGINT FROM rf),
               (SELECT (config->>'refresh_newest_first')::BOOLEAN FROM rf),
               (SELECT extract(epoch FROM (config->>'drop_after')::interval)::BIGINT
                  FROM timescaledb_information.jobs
                 WHERE proc_name = 'policy_retention' AND hypertable_name = $1)",
    )
    .bind(SWAPS_5MIN)
    .fetch_one(&pool)
    .await
    .expect("the 5-minute settings must be readable, one policy of each kind");

    assert_eq!(
        row,
        (
            Some(6 * DAY),
            Some(FREEZE_MARGIN.num_seconds()),
            Some(5 * 60),
            Some(false),
            Some(30 * DAY)
        )
    );
}

/// `(table, seconds)` for every row of `sql`, which leaves out `$1`.
async fn settings(pool: &PgPool, sql: &'static str, excluded: &str) -> Vec<(String, i64)> {
    sqlx::query_as(sql)
        .bind(excluded)
        .fetch_all(pool)
        .await
        .expect("the catalog must be readable")
}

/// Fails on every table whose setting is not `house(table)`. ⚠️ Also when the liquidity events
/// are missing: a filter matching nothing would pass with nothing checked.
fn assert_house(setting: &str, rows: &[(String, i64)], house: impl Fn(&str) -> i64) {
    assert!(
        rows.iter()
            .any(|(table, _)| table.starts_with("meteora_damm_v2_liquidity_events")),
        "{setting}: the read must reach the liquidity events, got {rows:?}"
    );
    let moved: Vec<&(String, i64)> = rows
        .iter()
        .filter(|(table, seconds)| *seconds != house(table))
        .collect();
    assert!(
        moved.is_empty(),
        "{setting} moved outside the swaps (seconds): {moved:?}"
    );
}
