//! `PgAnalyticRepository`, written under `yog_analytic` itself: the facts and
//! the protocol aggregate against the activity view, the freeze, the TVL.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{
    INSUFFICIENT_PRIVILEGE, as_role, assert_reads_no_password_hash, pk, price_mint, sqlstate,
};

/// One instant per test, at the precision Postgres keeps: two `Utc::now()`
/// on either side of an hour would put a swap and its cycle in different hours.
fn now() -> DateTime<Utc> {
    Utc::now()
        .duration_trunc(Duration::microseconds(1))
        .unwrap()
}

fn hours_before(now: DateTime<Utc>, hours: i64) -> DateTime<Utc> {
    (now - Duration::hours(hours))
        .duration_trunc(Duration::hours(1))
        .unwrap()
}

/// The cycle at `now` that writes every hour from `first`, and freezes the
/// closed ones.
fn cycle_from(now: DateTime<Utc>, first: DateTime<Utc>) -> CycleWrite {
    CycleWrite::plan(
        now,
        AnalyticProgress {
            first_hour: Some(first),
            last_frozen: None,
        },
    )
}

/// The two mints of the pool [`setup_pool`] builds from `seed`.
fn mints_of(seed: u8) -> (String, String) {
    (pk(seed + 100).to_string(), pk(seed + 200).to_string())
}

/// Token A unlisted, token B priced when `price_b` is given, as yog-context
/// leaves most pools.
async fn setup_pool(pool: &PgPool, seed: u8, price_b: Option<&str>) -> String {
    let address = pk(seed).to_string();
    let (mint_a, mint_b) = mints_of(seed);
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, 'meteora_damm_v2', $2, $3)",
    )
    .bind(&address)
    .bind(&mint_a)
    .bind(&mint_b)
    .execute(pool)
    .await
    .unwrap();
    for (mint, decimals) in [(&mint_a, 6i16), (&mint_b, 9i16)] {
        sqlx::query(
            "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
             VALUES ($1, $2, now(), now())",
        )
        .bind(mint)
        .bind(decimals)
        .execute(pool)
        .await
        .unwrap();
    }
    if let Some(price) = price_b {
        price_mint(pool, &mint_b, price).await;
    }
    address
}

/// A swap each way, both fees on the out token (`collect_fee_mode = 0`), each
/// carrying a protocol and a referral share.
async fn trade(pool: &PgPool, address: &str, tag: &str, at: DateTime<Utc>) {
    for (direction, a, b, claiming, protocol, referral, fee_is_a) in [
        (
            "a_to_b",
            1_000_000_000i64,
            500_000_000i64,
            4_000_000i64,
            800_000i64,
            200_000i64,
            false,
        ),
        (
            "b_to_a",
            2_000_000_000,
            1_000_000_000,
            80_000_000,
            16_000_000,
            4_000_000,
            true,
        ),
    ] {
        sqlx::query(
            "INSERT INTO meteora_damm_v2_swap_events
               (pool_address, signature, trade_direction,
                amount_a, amount_b, reserve_a_after, reserve_b_after, next_sqrt_price,
                claiming_fee, protocol_fee, compounding_fee, referral_fee, fee_token_is_a,
                timestamp, slot, event_index)
             VALUES ($1, $2, $3, $4, $5, 0, 0, 0, $6, $7, 0, $8, $9, $10, 0, 0)",
        )
        .bind(address)
        .bind(format!("{tag}_{direction}"))
        .bind(direction)
        .bind(a)
        .bind(b)
        .bind(claiming)
        .bind(protocol)
        .bind(referral)
        .bind(fee_is_a)
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
    }
}

type PoolHour = (
    String,
    DateTime<Utc>,
    i64,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
);

async fn view_pool_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<PoolHour> {
    sqlx::query_as(
        "SELECT pool_address, bucket, swap_count, volume_usd, fees_usd,
                lp_fees_usd, protocol_fees_usd, referral_fees_usd
           FROM meteora_damm_v2_pool_hourly_activity
          WHERE swap_count IS NOT NULL AND bucket <= $1
          ORDER BY pool_address, bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn fact_pool_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<PoolHour> {
    sqlx::query_as(
        "SELECT pool_address, bucket, swap_count, volume_in_a_usd + volume_in_b_usd,
                fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd
           FROM analytic_pool_hourly
          WHERE bucket <= $1
          ORDER BY pool_address, bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

type ProtocolHour = (
    DateTime<Utc>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    Option<BigDecimal>,
    i64,
    i64,
    i64,
);

async fn view_protocol_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<ProtocolHour> {
    sqlx::query_as(
        "SELECT bucket, SUM(volume_usd), SUM(fees_usd), SUM(lp_fees_usd),
                SUM(swap_count)::BIGINT, COUNT(*),
                COUNT(*) FILTER (WHERE volume_usd IS NOT NULL)
           FROM meteora_damm_v2_pool_hourly_activity
          WHERE swap_count IS NOT NULL AND bucket <= $1
          GROUP BY bucket
          ORDER BY bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn aggregate_protocol_hours(pool: &PgPool, through: DateTime<Utc>) -> Vec<ProtocolHour> {
    sqlx::query_as(
        "SELECT bucket, volume_usd, fees_usd, lp_fees_usd,
                swap_count, active_pools, active_pools_valued
           FROM analytic_protocol_hourly
          WHERE protocol = 'meteora_damm_v2' AND bucket <= $1
          ORDER BY bucket",
    )
    .bind(through)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Against any formula of the branch drifting from `swap_parts` (010): a
/// fee share swapped, a side dropped, a valued flag read from the wrong side.
#[sqlx::test]
async fn facts_and_aggregate_equal_the_activity_view_on_closed_hours(pool: PgPool) {
    let now = now();
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;
    trade(&pool, &priced, "p3", hours_before(now, 3)).await;
    trade(&pool, &priced, "p2", hours_before(now, 2)).await;
    trade(&pool, &unpriced, "u2", hours_before(now, 2)).await;
    let closed = hours_before(now, 2);

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    let cycle = cycle_from(now, hours_before(now, 3));
    repository.write_cycle(&cycle).await.unwrap();

    let view = view_pool_hours(&pool, closed).await;
    assert_eq!(view.len(), 3);
    assert_eq!(view.iter().filter(|h| h.3.is_some()).count(), 2, "{view:?}");
    assert!(view.iter().any(|h| h.7.as_ref().is_some_and(|r| r > 0)));
    assert_eq!(fact_pool_hours(&pool, closed).await, view);

    let by_protocol = view_protocol_hours(&pool, closed).await;
    assert_eq!(by_protocol.len(), 2);
    assert_eq!(aggregate_protocol_hours(&pool, closed).await, by_protocol);
}

/// Against a 5-minute tranche lost (the last of an hour), a sum dropped from
/// the rollup, the raw swaps past the watermark left out, or the implied price
/// dropped from the pricing both read.
#[sqlx::test]
async fn facts_summed_from_the_tranches_equal_the_hourly_view(pool: PgPool) {
    let open = hours_before(now(), 3);
    let freezing = open - Duration::hours(1);
    let implied = setup_pool(&pool, 1, Some("180.0")).await;
    let unvalued = setup_pool(&pool, 2, None).await;
    for (hour, tag) in [(freezing, "f"), (open, "o")] {
        for minute in [0, 25, 55] {
            let at = hour + Duration::minutes(minute) + Duration::seconds(30);
            trade(&pool, &implied, &format!("{tag}{minute}"), at).await;
        }
        trade(
            &pool,
            &unvalued,
            &format!("{tag}u"),
            hour + Duration::minutes(30),
        )
        .await;
    }
    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', $1, $2)")
        .bind(freezing)
        .bind(freezing + Duration::minutes(30))
        .execute(&pool)
        .await
        .unwrap();
    let watermark: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT watermark FROM yog_cagg_materialization_backlog()
          WHERE aggregate = 'meteora_damm_v2_swap_events_5min'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(watermark, Some(freezing + Duration::minutes(30)));

    let flags: (bool, bool, i64) = sqlx::query_as(
        "SELECT bool_and(price_a_implied) FILTER (WHERE pool_address = $1),
                bool_and(NOT valuation_complete) FILTER (WHERE pool_address = $2),
                count(*)
           FROM meteora_damm_v2_swap_events_hourly_priced",
    )
    .bind(&implied)
    .bind(&unvalued)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(flags, (true, true, 4));

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    let outcome = repository
        .write_cycle(&cycle_from(open + Duration::minutes(20), freezing))
        .await
        .unwrap();
    assert_eq!((outcome.pool_hours, outcome.hours_frozen), (4, 1));

    let view = view_pool_hours(&pool, open).await;
    let mut counts: Vec<i64> = view.iter().map(|h| h.2).collect();
    counts.sort();
    assert_eq!(counts, [2, 2, 6, 6], "{view:?}");
    let facts = fact_pool_hours(&pool, open).await;
    assert!(
        facts.iter().all(|h| h.3.is_some() == (h.0 == implied)),
        "the implied price values its pool's facts, and only those: {facts:?}"
    );
    assert_eq!(facts, view);
}

/// ☠️ Pins the loss the READMEs warn about: a late swap in a tranche already
/// materialised, frozen before the next refresh. If it stops, so must the warning.
#[sqlx::test]
async fn a_late_swap_in_a_materialised_tranche_misses_its_frozen_hour(pool: PgPool) {
    let hour = hours_before(now(), 3);
    let address = setup_pool(&pool, 1, Some("180.0")).await;
    trade(&pool, &address, "on_time", hour + Duration::minutes(20)).await;
    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', $1, $2)")
        .bind(hour)
        .bind(hour + Duration::hours(1))
        .execute(&pool)
        .await
        .unwrap();
    trade(&pool, &address, "late", hour + Duration::minutes(20)).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    let outcome = repository
        .write_cycle(&cycle_from(hour + Duration::minutes(80), hour))
        .await
        .unwrap();
    assert_eq!(outcome.hours_frozen, 1);

    assert_eq!(view_pool_hours(&pool, hour).await[0].2, 4);
    assert_eq!(fact_pool_hours(&pool, hour).await[0].2, 2);
}

/// Against the freeze guard of `write_cycle` removed: the frozen hour would
/// be rewritten with the late swap.
#[sqlx::test]
async fn a_swap_written_after_the_freeze_changes_nothing_of_its_hour(pool: PgPool) {
    let now = now();
    let address = setup_pool(&pool, 1, Some("180.0")).await;
    let frozen = hours_before(now, 3);
    trade(&pool, &address, "before", frozen).await;
    let current = hours_before(now, 0);
    trade(&pool, &address, "now", current).await;

    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    let cycle = cycle_from(now, frozen);
    let first = repository.write_cycle(&cycle).await.unwrap();
    assert!(first.hours_frozen >= 1);
    let facts = fact_pool_hours(&pool, frozen).await;
    let aggregate = aggregate_protocol_hours(&pool, frozen).await;
    assert_eq!(facts[0].2, 2);

    trade(&pool, &address, "late", frozen + Duration::minutes(30)).await;
    trade(&pool, &address, "later", current).await;
    repository.write_cycle(&cycle).await.unwrap();

    assert_eq!(fact_pool_hours(&pool, frozen).await, facts);
    assert_eq!(aggregate_protocol_hours(&pool, frozen).await, aggregate);
    let open = fact_pool_hours(&pool, current).await;
    assert_eq!(
        open.last().unwrap().2,
        4,
        "the open hour is rewritten: {open:?}"
    );
}

/// Returns once a statement of the test's database waits on a lock — the cycle
/// behind the other writer, wherever it blocks.
async fn wait_for_a_blocked_statement(pool: &PgPool) {
    for _ in 0..500 {
        let blocked: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks l JOIN pg_stat_activity a ON a.pid = l.pid
              WHERE NOT l.granted AND a.datname = current_database()",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if blocked > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the cycle never waited on the other writer");
}

/// Against the table lock of `write_cycle` removed: the cycle would read no
/// frozen hour, wait on the other writer's fact, then overwrite it.
#[sqlx::test]
async fn a_cycle_waits_for_a_concurrent_freeze_and_keeps_out_of_it(pool: PgPool) {
    let now = now();
    let address = setup_pool(&pool, 1, Some("180.0")).await;
    let frozen = hours_before(now, 3);
    trade(&pool, &address, "before", frozen).await;
    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);

    let mut other_writer = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO analytic_hours (bucket, frozen_at) VALUES ($1, now())")
        .bind(frozen)
        .execute(&mut *other_writer)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO analytic_pool_hourly (pool_address, bucket, protocol, swap_count, valued)
         VALUES ($1, $2, 'meteora_damm_v2', 999, false)",
    )
    .bind(&address)
    .bind(frozen)
    .execute(&mut *other_writer)
    .await
    .unwrap();

    let cycle = cycle_from(now, frozen);
    let writing = tokio::spawn(async move { repository.write_cycle(&cycle).await });
    wait_for_a_blocked_statement(&pool).await;
    other_writer.commit().await.unwrap();
    let outcome = writing.await.unwrap().unwrap();

    assert!(outcome.written.from > frozen, "{outcome:?}");
    assert_eq!(fact_pool_hours(&pool, frozen).await[0].2, 999);
}

#[sqlx::test]
async fn the_tvl_is_the_sum_of_pool_current_tvl(pool: PgPool) {
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;
    price_mint(&pool, &mints_of(1).0, "2.5").await;
    for (address, reserve_a, reserve_b) in
        [(&priced, 3_000_000i64, 7_000_000_000i64), (&unpriced, 1, 1)]
    {
        sqlx::query(
            "INSERT INTO pool_current_state
               (pool_address, protocol, last_event_at, last_event_kind, last_signature,
                reserve_a, reserve_b, last_slot, last_event_index)
             VALUES ($1, 'meteora_damm_v2', now(), 'liquidity_add', 'sig', $2, $3, 1, 0)",
        )
        .bind(address)
        .bind(reserve_a)
        .bind(reserve_b)
        .execute(&pool)
        .await
        .unwrap();
    }

    let now = now();
    let repository = PgAnalyticRepository::new(as_role(&pool, "yog_analytic").await);
    let cycle = cycle_from(now, hours_before(now, 0));
    repository.write_cycle(&cycle).await.unwrap();

    let expected: (Option<BigDecimal>, i64) = sqlx::query_as(
        "SELECT SUM(tvl_usd), COUNT(*) FILTER (WHERE tvl_usd IS NOT NULL) FROM pool_current_tvl",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(expected, (Some("1267.5".parse::<BigDecimal>().unwrap()), 1));
    let measured: (DateTime<Utc>, Option<BigDecimal>, i64, DateTime<Utc>) = sqlx::query_as(
        "SELECT bucket, tvl_usd, pools_priced, measured_at
           FROM analytic_protocol_tvl WHERE protocol = 'meteora_damm_v2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        measured,
        (cycle.tvl_hour.unwrap(), expected.0, expected.1, cycle.at)
    );
}

/// Against `yog_analytic` made a member of `pg_read_all_data`, which reads
/// every role's password hash.
#[sqlx::test]
async fn yog_analytic_cannot_read_the_password_hashes(pool: PgPool) {
    let analytic = as_role(&pool, "yog_analytic").await;

    assert_reads_no_password_hash(&mut analytic.acquire().await.unwrap()).await;
}

#[sqlx::test]
async fn yog_analytic_cannot_write_signals(pool: PgPool) {
    let analytic = as_role(&pool, "yog_analytic").await;

    let err = sqlx::query(
        "INSERT INTO signals (detector, protocol, pool_address, severity, value, triggered_at)
         VALUES ('d', 'meteora_damm_v2', 'p', 'warning', 1, now())",
    )
    .execute(&analytic)
    .await
    .expect_err("yog_analytic writes the analytic tables only");

    assert_eq!(sqlstate(&err), INSUFFICIENT_PRIVILEGE);
    let message = err.as_database_error().unwrap().message().to_owned();
    assert!(message.contains("table signals"), "{message}");
}
