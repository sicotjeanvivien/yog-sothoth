//! Integration tests for migration 014 — the raw swaps are chunked by the day,
//! compressed a day after their chunk closes, and kept 7 days.
//!
//! Gated behind `integration-tests`. Both tests read the TimescaleDB catalog
//! (`timescaledb_information.dimensions` and `.jobs`), which is where a later
//! migration re-declaring one of these settings lands. Intervals are read as
//! seconds: the text a job's config holds depends on the `IntervalStyle` in
//! effect when the policy was created.
//!
//! ⚠️ The policies never fire here (`max_background_workers = 0`): these tests
//! say what is declared, not what a running scheduler does with it.

use sqlx::{PgPool, Row};

const HOUR: i64 = 3_600;
const DAY: i64 = 24 * HOUR;

const SWAPS: &str = "meteora_damm_v2_swap_events";
const SWAPS_HOURLY: &str = "meteora_damm_v2_swap_events_hourly";

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
    assert_house("chunk_time_interval", &chunks, |_| 7 * DAY);

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
    assert_house("drop_after", &retention, |_| 30 * DAY);

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
    assert_house("start_offset", &refresh, |_| 29 * DAY);
}

/// `(table, seconds)` for every row of `sql`, which leaves out `$1`.
async fn settings(pool: &PgPool, sql: &'static str, excluded: &str) -> Vec<(String, i64)> {
    sqlx::query_as(sql)
        .bind(excluded)
        .fetch_all(pool)
        .await
        .expect("the catalog must be readable")
}

/// Fails on every table whose setting is not `house(table)`.
///
/// ⚠️ Also fails when the liquidity events are not among the rows: a filter
/// that matched nothing would otherwise pass with nothing checked.
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
