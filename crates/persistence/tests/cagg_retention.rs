//! The four retention policies against the four cagg refresh policies. ⚠️ `drop_chunks`
//! logs an invalidation, so a refresh reaching back past the retention recomputes a dropped
//! range from nothing and deletes its buckets: `start_offset < drop_after`. Asserted in the
//! catalog, reproduced at the instant the daily retention cuts, and — ☠️ on purpose — shown
//! destroying data through a full-range refresh the policy cannot constrain.

use super::helpers::pk;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row;

/// The four aggregates, by view name. Written out, so a fifth or a lost one fails here.
const EXPECTED_CAGGS: [&str; 4] = [
    "meteora_damm_v2_claim_position_fee_events_hourly",
    "meteora_damm_v2_claim_reward_events_hourly",
    "meteora_damm_v2_liquidity_events_hourly",
    "meteora_damm_v2_swap_events_hourly",
];

/// Each aggregate's refresh policy with its source hypertable's retention, linked by the catalog.
/// ⚠️ Retention LEFT joined: a source without one (`signals`) never loses rows, so the rule holds.
/// The refresh side stays INNER: there, a missing policy is the defect.
const POLICY_PAIRS_SQL: &str = "
    SELECT ca.view_name::TEXT                        AS cagg,
           ca.hypertable_name::TEXT                  AS raw_table,
           (rf.config->>'start_offset')                AS start_offset,
           (rt.config->>'drop_after')                  AS drop_after,
           (rf.config->>'start_offset')::interval
             < (rt.config->>'drop_after')::interval  AS refresh_stays_inside
      FROM timescaledb_information.continuous_aggregates ca
      JOIN timescaledb_information.jobs rf
        ON rf.proc_name = 'policy_refresh_continuous_aggregate'
       AND rf.hypertable_name = ca.view_name
      LEFT JOIN timescaledb_information.jobs rt
        ON rt.proc_name = 'policy_retention'
       AND rt.hypertable_name = ca.hypertable_name
     ORDER BY ca.view_name";

#[sqlx::test]
async fn every_refresh_window_stays_inside_its_retention(pool: PgPool) {
    let rows = sqlx::query(POLICY_PAIRS_SQL)
        .fetch_all(&pool)
        .await
        .expect("the policy pairs must be readable from the catalog");

    // ⚠️ Counted apart from the join: a cagg without a refresh policy drops out of the INNER
    // side, and only a direct count of the catalog sees it.
    let declared: i64 = sqlx::query_scalar(
        "SELECT count(*)::BIGINT FROM timescaledb_information.continuous_aggregates",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        declared as usize,
        EXPECTED_CAGGS.len(),
        "a continuous aggregate exists that this test does not know about — add \
         it to EXPECTED_CAGGS, and give it a refresh policy before you do"
    );

    let found: Vec<String> = rows.iter().map(|r| r.get::<String, _>("cagg")).collect();
    assert_eq!(
        found, EXPECTED_CAGGS,
        "every continuous aggregate must have a refresh policy — one without is \
         an aggregate that never materializes a bucket, and it drops out of the \
         INNER join above, which is why the count is taken separately"
    );

    for row in &rows {
        let cagg: String = row.get("cagg");
        let raw_table: String = row.get("raw_table");
        let start_offset: String = row.get("start_offset");
        let drop_after: Option<String> = row.get("drop_after");
        let stays_inside: Option<bool> = row.get("refresh_stays_inside");

        let Some(drop_after) = drop_after else {
            continue;
        };

        assert_eq!(
            stays_inside,
            Some(true),
            "{cagg}: start_offset ({start_offset}) must stay STRICTLY under \
             {raw_table}'s drop_after ({drop_after}). A refresh reaching past \
             the retention recomputes a range whose raw rows are gone and \
             deletes the materialized buckets — see migration 008."
        );
    }
}

/// The instant the daily retention first drops a chunk, and the window the policy refreshes
/// then. ⚠️ Anchored on a real chunk boundary, not `now()`: a chunk drops only once entirely
/// older than `drop_after`, so a `now()`-based fixture can miss the regression and stay green.
struct RetentionMoment {
    /// The cut: chunks ending at or before this are dropped.
    boundary: DateTime<Utc>,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
}

async fn retention_moment(pool: &PgPool, cagg: &str, raw_table: &str) -> RetentionMoment {
    let row = sqlx::query(
        "WITH b AS (
             SELECT min(range_end) AS boundary
               FROM timescaledb_information.chunks
              WHERE hypertable_name = $2
         ),
         p AS (
             SELECT (rf.config->>'start_offset')::interval AS start_offset,
                    (rf.config->>'end_offset')::interval   AS end_offset,
                    (rt.config->>'drop_after')::interval   AS drop_after
               FROM timescaledb_information.jobs rf
               JOIN timescaledb_information.jobs rt
                 ON rt.proc_name = 'policy_retention'
                AND rt.hypertable_name = $2
              WHERE rf.proc_name = 'policy_refresh_continuous_aggregate'
                AND rf.hypertable_name = $1
         )
         SELECT b.boundary,
                b.boundary + p.drop_after - p.start_offset AS window_start,
                b.boundary + p.drop_after - p.end_offset   AS window_end
           FROM b, p",
    )
    .bind(cagg)
    .bind(raw_table)
    .fetch_one(pool)
    .await
    .expect("the chunk boundary and both policies must be readable");

    RetentionMoment {
        boundary: row.get("boundary"),
        window_start: row.get("window_start"),
        window_end: row.get("window_end"),
    }
}

/// Buckets currently materialized — not what a read returns, which adds the raw rows. The
/// `materialized_only` flag is flipped for the count and put back.
async fn materialized_buckets(pool: &PgPool) -> (i64, Option<DateTime<Utc>>) {
    sqlx::query(
        "ALTER MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
         SET (timescaledb.materialized_only = true)",
    )
    .execute(pool)
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT count(*)::BIGINT AS n, min(bucket) AS oldest
           FROM meteora_damm_v2_swap_events_hourly",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "ALTER MATERIALIZED VIEW meteora_damm_v2_swap_events_hourly
         SET (timescaledb.materialized_only = false)",
    )
    .execute(pool)
    .await
    .unwrap();

    (row.get("n"), row.get("oldest"))
}

#[sqlx::test]
async fn the_policy_refresh_cannot_erase_what_retention_dropped(pool: PgPool) {
    let addr = pk(1).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&addr)
    .bind(pk(2).to_string())
    .bind(pk(3).to_string())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         SELECT $1, 'sig-' || h, 'a_to_b', 1000, 1000, 0, 0, 0, 10, 0, 0, 0, true,
                now() - (h || ' hours')::interval, 0, 0
           FROM generate_series(1, 90 * 24) h",
    )
    .bind(&addr)
    .execute(&pool)
    .await
    .unwrap();

    // `refresh_continuous_aggregate` cannot run inside a transaction; `sqlx::test` hands out a pool.
    sqlx::query(
        "CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_hourly', NULL, NULL)",
    )
    .execute(&pool)
    .await
    .expect("a full refresh must materialize the seeded history");

    let (before, oldest_before) = materialized_buckets(&pool).await;
    assert_eq!(before, 90 * 24, "the 90 seeded days must all materialize");

    let moment = retention_moment(
        &pool,
        "meteora_damm_v2_swap_events_hourly",
        "meteora_damm_v2_swap_events",
    )
    .await;

    let raw_before: i64 =
        sqlx::query_scalar("SELECT count(*)::BIGINT FROM meteora_damm_v2_swap_events")
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::query("SELECT drop_chunks('meteora_damm_v2_swap_events', older_than => $1)")
        .bind(moment.boundary)
        .execute(&pool)
        .await
        .expect("drop_chunks must run");

    let raw_after: i64 =
        sqlx::query_scalar("SELECT count(*)::BIGINT FROM meteora_damm_v2_swap_events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        raw_after < raw_before,
        "the fixture must actually lose raw rows, or the rest of this test \
         passes for the wrong reason — that is how its first version stayed \
         green with the defect in place"
    );

    let (after_drop, _) = materialized_buckets(&pool).await;
    assert_eq!(
        after_drop, before,
        "dropping raw chunks must not touch the aggregate — that is the whole \
         point of a cagg, and it held even before migration 008"
    );

    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_hourly', $1, $2)")
        .bind(moment.window_start)
        .bind(moment.window_end)
        .execute(&pool)
        .await
        .expect("the policy's own refresh must run");

    let (after_refresh, oldest_after) = materialized_buckets(&pool).await;
    assert_eq!(
        after_refresh, before,
        "the refresh reached past the retention cut, recomputed a range whose \
         raw rows were gone, and wrote back the nothing it found. With \
         start_offset one day beyond drop_after this loses up to 24 buckets — \
         up to one day of history per chunk dropped, for ever."
    );
    assert_eq!(
        oldest_after, oldest_before,
        "the aggregate's history must not lose its oldest end — that history \
         surviving the raw retention is the reason the cagg exists"
    );
}

#[sqlx::test]
async fn a_full_range_refresh_still_destroys_the_history(pool: PgPool) {
    // ☠️ Asserts a destruction on purpose: a full-range refresh processes every accumulated
    // invalidation. If TimescaleDB stops doing so, this fails — then delete the warning, not the test.
    let addr = pk(1).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1,'meteora_damm_v2',$2,$3)",
    )
    .bind(&addr)
    .bind(pk(2).to_string())
    .bind(pk(3).to_string())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         SELECT $1, 'sig-' || h, 'a_to_b', 1000, 1000, 0, 0, 0, 10, 0, 0, 0, true,
                now() - (h || ' hours')::interval, 0, 0
           FROM generate_series(1, 90 * 24) h",
    )
    .bind(&addr)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_hourly', NULL, NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (before, _) = materialized_buckets(&pool).await;

    sqlx::query(
        "SELECT drop_chunks('meteora_damm_v2_swap_events', older_than => INTERVAL '30 days')",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_hourly', NULL, NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (after, _) = materialized_buckets(&pool).await;

    assert!(
        after < before,
        "a full-range refresh over dropped raw data is expected to DESTROY \
         history ({before} buckets before, {after} after). If this ever stops \
         being true, the ☠️ warning in migrations/README.md can go — check, \
         do not assume."
    );
}
