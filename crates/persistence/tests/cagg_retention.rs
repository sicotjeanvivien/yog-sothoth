//! Integration tests for the relation between the four retention policies and
//! the four continuous-aggregate refresh policies (migration 008).
//!
//! Gated behind `integration-tests`. The finding they guard (`.project` ticket
//! 03, re-diagnosed on 10 August 2026): a refresh whose window reaches further
//! back than the retention **deletes** materialized buckets.
//!
//! `drop_chunks` logs an invalidation over the range it removes. A refresh is
//! invalidation-driven, so a window containing that invalidation recomputes the
//! range from raw rows that are gone and writes the result — nothing. Measured
//! on a throwaway database: 288 materialized buckets, retention drop, ONE
//! refresh over `[now-31d, now-1h]`, 130 left.
//!
//! The rule is therefore `start_offset < drop_after`, and it is asserted twice:
//! once as itself, read out of the TimescaleDB catalog, and once through the
//! behaviour it exists for. The rule alone would not say *why* it is the right
//! rule; the behaviour alone would not say which knob to turn.

use super::helpers::pk;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::Row;

/// The four aggregates, in the order the catalog returns them (view name
/// ascending). Written out rather than derived, so that adding a fifth cagg —
/// or losing one of these — fails here instead of silently shrinking the set
/// the rule is checked over.
const EXPECTED_CAGGS: [&str; 4] = [
    "meteora_damm_v2_claim_position_fee_events_hourly",
    "meteora_damm_v2_claim_reward_events_hourly",
    "meteora_damm_v2_liquidity_events_hourly",
    "meteora_damm_v2_swap_events_hourly",
];

/// Pairs each aggregate's refresh policy with the retention policy of the raw
/// hypertable it reads. The pairing comes from
/// `timescaledb_information.continuous_aggregates`, not from a naming
/// convention: `view_name` → `hypertable_name` is the catalog's own link.
///
/// The interval comparison itself is left to Postgres — it is the one that
/// knows how to order `INTERVAL`s — and comes back as a boolean next to the two
/// values in their readable form.
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
      JOIN timescaledb_information.jobs rt
        ON rt.proc_name = 'policy_retention'
       AND rt.hypertable_name = ca.hypertable_name
     ORDER BY ca.view_name";

#[sqlx::test]
async fn every_refresh_window_stays_inside_its_retention(pool: PgPool) {
    let rows = sqlx::query(POLICY_PAIRS_SQL)
        .fetch_all(&pool)
        .await
        .expect("the policy pairs must be readable from the catalog");

    let found: Vec<String> = rows.iter().map(|r| r.get::<String, _>("cagg")).collect();
    assert_eq!(
        found, EXPECTED_CAGGS,
        "every continuous aggregate must have BOTH a refresh policy and a \
         retention policy on its source hypertable — a missing one drops out of \
         the join and takes its pair out of this check"
    );

    for row in &rows {
        let cagg: String = row.get("cagg");
        let raw_table: String = row.get("raw_table");
        let start_offset: String = row.get("start_offset");
        let drop_after: String = row.get("drop_after");
        let stays_inside: bool = row.get("refresh_stays_inside");

        assert!(
            stays_inside,
            "{cagg}: start_offset ({start_offset}) must stay STRICTLY under \
             {raw_table}'s drop_after ({drop_after}). A refresh reaching past \
             the retention recomputes a range whose raw rows are gone and \
             deletes the materialized buckets — see migration 008."
        );
    }
}

// ── The behaviour the rule exists for ────────────────────────────────────────

/// The instant the daily retention job would first drop a chunk, and the
/// refresh window the policy would use at that instant.
///
/// ## Why this is anchored on a chunk boundary and not on `now()`
///
/// A first version of this test seeded 40 days, called `drop_chunks(older_than
/// => '30 days')` and refreshed over `[now-31d, now-1h]` — and it stayed green
/// with `start_offset` back at 31 days, which is exactly the regression it was
/// written to catch. The reason is chunk geometry: raw chunks span 7 days and
/// are dropped only once **entirely** older than `drop_after`, so a single
/// manual drop clears data 30 to 37 days old depending on alignment. Measured
/// on that fixture, the youngest dropped row was 32 days old — a 31-day window
/// never reached it, and nothing was destroyed.
///
/// In production the alignment is not a lottery, because retention runs
/// **daily**: a chunk is dropped at the first run after its end crosses
/// `drop_after`, so at that moment its newest data is between `drop_after` and
/// `drop_after + 1 day` old — inside a window that reaches one day further
/// back. The loss is therefore not occasional, it is one per chunk.
///
/// This helper reproduces that instant instead of waiting for it: it takes a
/// real chunk boundary `B`, treats `B + drop_after` as "now", and returns the
/// window the policy would then compute. Measured on a probe with the real
/// 7-day geometry: overshooting the cut by a day erased **24 buckets — exactly
/// one day**; stopping a day short of it erased none ("already up-to-date").
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

/// Buckets currently **materialized** — not what a read returns, which unions
/// them with a live query over the raw rows (`materialized_only = false`). The
/// flag is flipped for the duration of the count and put back; the database is
/// this test's own, created and dropped by `sqlx::test`.
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

    // 90 days, one swap an hour — enough to span several 7-day chunks on both
    // sides of the retention cut.
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

    // `refresh_continuous_aggregate` cannot run inside a transaction, which is
    // why the migrations declare a policy instead of calling it (baseline §13).
    // `sqlx::test` hands out a pool, not a transaction, so it runs here.
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

    // The refresh the policy itself would run at that instant.
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
         start_offset one day beyond drop_after this loses exactly 24 buckets — \
         one day of history per chunk dropped, for ever."
    );
    assert_eq!(
        oldest_after, oldest_before,
        "the aggregate's history must not lose its oldest end — that history \
         surviving the raw retention is the reason the cagg exists"
    );
}
