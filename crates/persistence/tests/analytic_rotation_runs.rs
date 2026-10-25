//! The rotation runs of `PgAnalyticRepository`: each tranche's quoted side turned over, read
//! in the raw swaps; the tranches marked at `RUN_ROTATION` drawn into rotation runs `RUN_GAP`
//! apart, rewritten until settled, then written once with their hours.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{pk, price_at, setup_roles_and_connect_as, sqrt_price_x64};

const SOL: &str = "So11111111111111111111111111111111111111112";

/// The start of the hour three hours back: every tranche of it settled.
fn settled_hour() -> DateTime<Utc> {
    (Utc::now() - Duration::hours(3))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

fn at(base: DateTime<Utc>, minutes: i64) -> DateTime<Utc> {
    base + Duration::minutes(minutes)
}

/// A pool of `a` (6 decimals) against `b` (9 decimals), both at a market price.
async fn setup_pool(pool: &PgPool, seed: u8, a: &str, b: &str, priced_at: DateTime<Utc>) -> String {
    let address = pk(seed).to_string();
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(a)
    .bind(b)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals, price) in [(a, 6i16, "1"), (b, 9i16, "100")] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, now(), now())
             ON CONFLICT (mint) DO NOTHING",
        )
        .bind(mint)
        .bind(decimals)
        .execute(pool)
        .await
        .unwrap();
        price_at(pool, mint, price, priced_at).await;
    }
    address
}

/// A swap of `a` for `b`, leaving the reserves `(reserve_a, reserve_b)`.
async fn insert_swap(
    pool: &PgPool,
    address: &str,
    tag: &str,
    (a, b): (i64, i64),
    (reserve_a, reserve_b): (i64, i64),
    at: DateTime<Utc>,
) {
    sqlx::query(
        "INSERT INTO meteora_damm_v2_swap_events
           (pool_address, signature, trade_direction,
            amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
            claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
            timestamp, slot, event_index)
         VALUES ($1, $2, 'a_to_b', $3, $4, $5, $6, $7, 0, 0, 0, 0, false, $8, 0, 0)",
    )
    .bind(address)
    .bind(tag)
    .bind(a)
    .bind(b)
    .bind(reserve_a)
    .bind(reserve_b)
    .bind(sqrt_price_x64(a, b))
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

/// A tranche already valued, its rotation given: what the rotation runs read.
async fn insert_tranche(
    pool: &PgPool,
    address: &str,
    bucket: DateTime<Utc>,
    rotation: &str,
    volume: Option<&str>,
) {
    sqlx::query(
        "INSERT INTO analytic_pool_5min
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd,
            volume_in_b_usd, fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd, rotation)
         SELECT $1, $2, 'meteora_damm_v2', 1, $3, $4::NUMERIC, z.usd, z.usd, z.usd, z.usd,
                 z.usd, $5::NUMERIC
           FROM (SELECT CASE WHEN $3 THEN 0 END AS usd) z",
    )
    .bind(address)
    .bind(bucket)
    .bind(volume.is_some())
    .bind(volume)
    .bind(rotation)
    .execute(pool)
    .await
    .unwrap();
}

/// Values the tranches from `first` through `last`, at the first instant the last one may be.
async fn value(pool: &PgPool, first: DateTime<Utc>, last: DateTime<Utc>) {
    let progress = AnalyticProgress {
        first_hour: Some(first.duration_trunc(Duration::hours(1)).unwrap()),
        last_frozen: None,
        last_tranche: Some(first - Duration::minutes(5)),
    };
    PgAnalyticRepository::new(setup_roles_and_connect_as(pool, "yog_analytic").await)
        .write_cycle(&CycleWrite::plan(last + Duration::minutes(15), progress))
        .await
        .unwrap();
}

async fn rotation(pool: &PgPool, address: &str, bucket: DateTime<Utc>) -> Option<BigDecimal> {
    sqlx::query_scalar(
        "SELECT rotation FROM analytic_pool_5min WHERE pool_address = $1 AND bucket = $2",
    )
    .bind(address)
    .bind(bucket)
    .fetch_one(pool)
    .await
    .unwrap()
}

type RotationRun = (DateTime<Utc>, DateTime<Utc>, BigDecimal, bool, bool);

async fn rotation_runs(pool: &PgPool) -> Vec<RotationRun> {
    sqlx::query_as(
        "SELECT started_at, ended_at, volume_usd, extinct, settled
           FROM analytic_rotation_runs ORDER BY started_at",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Mutation: the rotation of the token side — 2 000 traded over a mean reserve of 100 would
/// read 20.
#[sqlx::test]
async fn a_tranche_turns_its_quoted_side_over(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let address = setup_pool(&pool, 1, &pk(10).to_string(), SOL, tranche).await;
    insert_swap(&pool, &address, "s1", (1_000, 200), (100, 100), tranche).await;
    insert_swap(&pool, &address, "s2", (1_000, 100), (100, 200), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(rotation(&pool, &address, tranche).await, Some(dec("2")));
}

/// Mutation: a pool whose mints are not resolved yet taken as a pair without a quoted side —
/// its rotation, written once, would be the smaller of its two.
#[sqlx::test]
async fn a_pool_with_its_mints_unresolved_has_no_rotation(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let address = pk(1).to_string();
    sqlx::query("INSERT INTO pools (pool_address, protocol) VALUES ($1, 'meteora_damm_v2')")
        .bind(&address)
        .execute(&pool)
        .await
        .unwrap();
    insert_swap(&pool, &address, "s1", (2_000, 300), (100, 150), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(rotation(&pool, &address, tranche).await, None);
}

/// Mutation: the larger rotation of a pair without a quoted side — the thin side would mark it.
#[sqlx::test]
async fn a_pair_without_a_quoted_side_takes_its_smaller_rotation(pool: PgPool) {
    let tranche = settled_hour() + Duration::minutes(10);
    let address = setup_pool(&pool, 1, &pk(10).to_string(), &pk(11).to_string(), tranche).await;
    insert_swap(&pool, &address, "s1", (2_000, 300), (100, 150), tranche).await;

    value(&pool, tranche, tranche).await;

    assert_eq!(rotation(&pool, &address, tranche).await, Some(dec("2")));
}

/// Mutation: `RUN_GAP` at 35 min — the two rotation runs would read as one; `RUN_ROTATION` under
/// 0.99 — a third rotation run at 120.
#[sqlx::test]
async fn marked_tranches_at_most_the_gap_apart_make_one_rotation_run(pool: PgPool) {
    let base = settled_hour();
    let address = pk(1).to_string();
    for (minutes, rotation) in [(0, "2"), (15, "0.1"), (30, "1"), (65, "3"), (120, "0.99")] {
        insert_tranche(&pool, &address, at(base, minutes), rotation, Some("10")).await;
    }

    value(&pool, base, at(base, 170)).await;

    assert_eq!(
        rotation_runs(&pool).await,
        vec![
            (base, at(base, 35), dec("30"), true, true),
            (at(base, 65), at(base, 70), dec("10"), true, true),
        ],
    );
}

/// Mutation: a rotation run written extinct, or settled, before `RUN_GAP` has passed its last
/// marked tranche — it would be final while it may still grow.
#[sqlx::test]
async fn a_rotation_run_in_progress_is_rewritten_until_it_settles(pool: PgPool) {
    let base = settled_hour();
    let address = pk(1).to_string();
    for minutes in [0, 65] {
        insert_tranche(&pool, &address, at(base, minutes), "2", Some("10")).await;
    }

    value(&pool, base, at(base, 90)).await;
    assert_eq!(
        rotation_runs(&pool).await,
        vec![
            (base, at(base, 5), dec("10"), true, true),
            (at(base, 65), at(base, 70), dec("10"), false, false),
        ],
    );

    insert_tranche(&pool, &address, at(base, 120), "2", Some("10")).await;
    value(&pool, at(base, 95), at(base, 175)).await;
    assert_eq!(
        rotation_runs(&pool).await,
        vec![
            (base, at(base, 5), dec("10"), true, true),
            (at(base, 65), at(base, 70), dec("10"), true, true),
            (at(base, 120), at(base, 125), dec("10"), true, true),
        ],
    );
}

/// Mutation: an extinct rotation run settled before its last hour is written whole — a tranche of
/// that hour, unvalued later, would leave its volume counted in the rotation run and not in the
/// hour.
#[sqlx::test]
async fn a_rotation_run_settles_once_its_last_hour_is_whole_and_counts_it_only_valued_whole(
    pool: PgPool,
) {
    let base = settled_hour();
    let address = pk(1).to_string();
    insert_tranche(&pool, &address, at(base, 0), "2", Some("10")).await;
    insert_tranche(&pool, &address, at(base, 40), "0.1", None).await;

    value(&pool, base, at(base, 50)).await;
    assert_eq!(
        rotation_runs(&pool).await,
        vec![(base, at(base, 5), dec("0"), true, false)],
    );

    value(&pool, at(base, 55), at(base, 55)).await;
    assert_eq!(
        rotation_runs(&pool).await,
        vec![(base, at(base, 5), dec("0"), true, true)],
    );
}

/// Mutation: a rotation run's volume put in the hour it starts — the curve would show none in the
/// next.
#[sqlx::test]
async fn a_rotation_runs_volume_is_split_by_hour(pool: PgPool) {
    let base = settled_hour();
    let address = pk(1).to_string();
    for minutes in [45, 55, 65] {
        insert_tranche(&pool, &address, at(base, minutes), "2", Some("10")).await;
    }

    value(&pool, at(base, 45), at(base, 100)).await;
    value(&pool, at(base, 105), at(base, 115)).await;
    value(&pool, at(base, 120), at(base, 120)).await;

    let hours: Vec<(DateTime<Utc>, BigDecimal)> = sqlx::query_as(
        "SELECT bucket, volume_usd FROM analytic_rotation_run_hours ORDER BY bucket",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(hours, vec![(base, dec("20")), (at(base, 60), dec("10"))],);
    assert_eq!(
        rotation_runs(&pool).await,
        vec![(at(base, 45), at(base, 70), dec("30"), true, true)],
    );
}

/// Mutation: a settled rotation run known by its start — once the retention drops its first
/// tranche, it would be written a second time from the next one.
#[sqlx::test]
async fn a_settled_rotation_run_is_not_drawn_again_once_its_first_tranches_are_dropped(
    pool: PgPool,
) {
    let base = settled_hour();
    let address = pk(1).to_string();
    for minutes in [45, 55, 65] {
        insert_tranche(&pool, &address, at(base, minutes), "2", Some("10")).await;
    }
    value(&pool, at(base, 45), at(base, 115)).await;

    sqlx::query("DELETE FROM analytic_pool_5min WHERE bucket = $1")
        .bind(at(base, 45))
        .execute(&pool)
        .await
        .unwrap();
    value(&pool, at(base, 120), at(base, 120)).await;

    assert_eq!(
        rotation_runs(&pool).await,
        vec![(at(base, 45), at(base, 70), dec("30"), true, true)],
    );
}
