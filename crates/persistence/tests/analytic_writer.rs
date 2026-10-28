//! `PgAnalyticRepository`, written under `yog_analytic` itself: the facts and
//! the protocol aggregate against what each trade is worth, the freeze, the TVL.

use chrono::{DateTime, Duration, DurationRound, Utc};
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use yog_core::domain::{AnalyticProgress, AnalyticRepository, CycleWrite};
use yog_persistence::PgAnalyticRepository;

use super::helpers::{
    INSUFFICIENT_PRIVILEGE, assert_reads_no_password_hash, pk, price_mint,
    setup_roles_and_connect_as, sqlstate, sqrt_price_x64,
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
            last_tranche: None,
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
/// carrying a protocol and a referral share. ⚠️ A square rate: the pool's price
/// from its square root is then exact, and equal to the implied one.
async fn insert_trade(pool: &PgPool, address: &str, tag: &str, at: DateTime<Utc>) {
    for (direction, a, b, claiming, protocol, referral, fee_is_a) in [
        (
            "a_to_b",
            1_000_000_000i64,
            250_000_000i64,
            4_000_000i64,
            800_000i64,
            200_000i64,
            false,
        ),
        (
            "b_to_a",
            2_000_000_000,
            500_000_000,
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
             VALUES ($1, $2, $3, $4, $5, 0, 0, $11, $6, $7, 0, $8, $9, $10, 0, 0)",
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
        .bind(sqrt_price_x64(a, b))
        .execute(pool)
        .await
        .unwrap();
    }
}

fn dec(value: &str) -> BigDecimal {
    value.parse().unwrap()
}

/// What `trades` calls of [`trade`] are worth, B at $180: each sale its 0.25 B received and
/// 0.005 B of fee, each purchase its 0.5 B paid, its 100 A of fee their share of 2 100 A gross.
fn worth(trades: u32) -> [BigDecimal; 5] {
    let n = BigDecimal::from(trades);
    let in_b = |amount: &str| dec(amount) * dec("180");
    let in_a = |units: i64| BigDecimal::from(units) * dec("90") / dec("2100");
    let fees = in_b("0.005") + in_a(100);
    let protocol = in_b("0.0008") + in_a(16);
    let referral = in_b("0.0002") + in_a(4);
    let lp = &fees - &protocol - &referral;
    [in_b("0.755"), fees, lp, protocol, referral].map(|v| (v * &n).round(12))
}

/// The facts of `priced` worth `trades` trades an hour, and the others unvalued.
fn assert_worth(facts: &[PoolHour], priced: &str, trades: u32) {
    for fact in facts {
        let values = [&fact.3, &fact.4, &fact.5, &fact.6, &fact.7];
        if fact.0 == priced {
            let got = values.map(|v| v.as_ref().expect("a valued fact").round(12));
            assert_eq!(got, worth(trades), "{fact:?}");
        } else {
            assert!(values.iter().all(|v| v.is_none()), "{fact:?}");
        }
    }
}

/// `(pool, hour, swaps, valued)` of each pool-hour: what the view and the facts must share.
fn keys(hours: &[PoolHour]) -> Vec<(String, DateTime<Utc>, i64, bool)> {
    hours
        .iter()
        .map(|h| (h.0.clone(), h.1, h.2, h.3.is_some()))
        .collect()
}

/// The protocol hours the facts sum to.
fn summed(facts: &[PoolHour]) -> Vec<ProtocolHour> {
    let mut hours: Vec<ProtocolHour> = Vec::new();
    for fact in facts {
        let at = match hours.iter().position(|h| h.0 == fact.1) {
            Some(at) => at,
            None => {
                hours.push((fact.1, None, None, None, 0, 0, 0));
                hours.len() - 1
            }
        };
        let hour = &mut hours[at];
        let add = |sum: &mut Option<BigDecimal>, v: &Option<BigDecimal>| {
            if let Some(v) = v {
                *sum = Some(sum.take().unwrap_or_default() + v);
            }
        };
        add(&mut hour.1, &fact.3);
        add(&mut hour.2, &fact.4);
        add(&mut hour.3, &fact.5);
        hour.4 += fact.2;
        hour.5 += 1;
        hour.6 += i64::from(fact.3.is_some());
    }
    hours.sort_by_key(|h| h.0);
    hours
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

/// Against a formula of the branch drifting: a fee share swapped, a side dropped, a
/// valued flag read from the wrong side. The view gives the hours and their swaps.
#[sqlx::test]
async fn facts_and_aggregate_equal_the_activity_view_on_closed_hours(pool: PgPool) {
    let now = now();
    let priced = setup_pool(&pool, 1, Some("180.0")).await;
    let unpriced = setup_pool(&pool, 2, None).await;
    insert_trade(&pool, &priced, "p3", hours_before(now, 3)).await;
    insert_trade(&pool, &priced, "p2", hours_before(now, 2)).await;
    insert_trade(&pool, &unpriced, "u2", hours_before(now, 2)).await;
    let closed = hours_before(now, 2);

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    let cycle = cycle_from(now, hours_before(now, 3));
    repository.write_cycle(&cycle).await.unwrap();

    let view = view_pool_hours(&pool, closed).await;
    assert_eq!(view.len(), 3);
    assert_eq!(view.iter().filter(|h| h.3.is_some()).count(), 2, "{view:?}");
    let facts = fact_pool_hours(&pool, closed).await;
    assert_eq!(keys(&facts), keys(&view));
    assert_worth(&facts, &priced, 1);

    assert_eq!(view_protocol_hours(&pool, closed).await.len(), 2);
    assert_eq!(
        aggregate_protocol_hours(&pool, closed).await,
        summed(&facts)
    );
}

/// Against a 5-minute tranche lost (the last of an hour), a sum dropped from
/// the rollup, the raw swaps past the watermark left out, or the fee in A valued
/// otherwise than as its share of the purchase.
#[sqlx::test]
async fn facts_summed_from_the_tranches_equal_the_hourly_view(pool: PgPool) {
    let open = hours_before(now(), 3);
    let freezing = open - Duration::hours(1);
    let implied = setup_pool(&pool, 1, Some("180.0")).await;
    let unvalued = setup_pool(&pool, 2, None).await;
    for (hour, tag) in [(freezing, "f"), (open, "o")] {
        for minute in [0, 25, 55] {
            let at = hour + Duration::minutes(minute) + Duration::seconds(30);
            insert_trade(&pool, &implied, &format!("{tag}{minute}"), at).await;
        }
        insert_trade(
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

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    let outcome = repository
        .write_cycle(&cycle_from(open + Duration::minutes(70), freezing))
        .await
        .unwrap();
    assert_eq!((outcome.pool_hours, outcome.hours_frozen), (4, 2));

    let view = view_pool_hours(&pool, open).await;
    let mut counts: Vec<i64> = view.iter().map(|h| h.2).collect();
    counts.sort();
    assert_eq!(counts, [2, 2, 6, 6], "{view:?}");
    let facts = fact_pool_hours(&pool, open).await;
    assert_eq!(keys(&facts), keys(&view));
    assert_worth(&facts, &implied, 3);
}

/// ☠️ Pins the loss the READMEs warn about: a late swap in a tranche already
/// materialised, frozen before the next refresh. If it stops, so must the warning.
#[sqlx::test]
async fn a_late_swap_in_a_materialised_tranche_misses_its_frozen_hour(pool: PgPool) {
    let hour = hours_before(now(), 3);
    let address = setup_pool(&pool, 1, Some("180.0")).await;
    insert_trade(&pool, &address, "on_time", hour + Duration::minutes(20)).await;
    sqlx::query("CALL refresh_continuous_aggregate('meteora_damm_v2_swap_events_5min', $1, $2)")
        .bind(hour)
        .bind(hour + Duration::hours(1))
        .execute(&pool)
        .await
        .unwrap();
    insert_trade(&pool, &address, "late", hour + Duration::minutes(20)).await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
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
    let open = hours_before(now(), 1);
    let frozen = open - Duration::hours(2);
    let address = setup_pool(&pool, 1, Some("180.0")).await;
    insert_trade(&pool, &address, "before", frozen).await;
    insert_trade(&pool, &address, "now", open + Duration::minutes(10)).await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    let first = repository
        .write_cycle(&cycle_from(open + Duration::minutes(20), frozen))
        .await
        .unwrap();
    assert!(first.hours_frozen >= 1);
    let facts = fact_pool_hours(&pool, frozen).await;
    let aggregate = aggregate_protocol_hours(&pool, frozen).await;
    assert_eq!(facts[0].2, 2);

    insert_trade(&pool, &address, "late", frozen + Duration::minutes(30)).await;
    insert_trade(&pool, &address, "later", open + Duration::minutes(10)).await;
    let progress = repository.progress().await.unwrap();
    repository
        .write_cycle(&CycleWrite::plan(open + Duration::minutes(30), progress))
        .await
        .unwrap();

    assert_eq!(fact_pool_hours(&pool, frozen).await, facts);
    assert_eq!(aggregate_protocol_hours(&pool, frozen).await, aggregate);
    let open = fact_pool_hours(&pool, open).await;
    assert_eq!(
        open.last().unwrap().2,
        4,
        "a tranche not yet valued takes the later swap: {open:?}"
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
    insert_trade(&pool, &address, "before", frozen).await;
    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);

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

/// The pool's current reserves, as the indexer leaves them after an event.
async fn hold(pool: &PgPool, address: &str, reserve_a: i64, reserve_b: i64) {
    sqlx::query(
        "INSERT INTO pool_current_state
           (pool_address, protocol, last_event_at, last_event_kind, last_signature,
            reserve_a, reserve_b, last_slot, last_event_index)
         VALUES ($1, 'meteora_damm_v2', now(), 'liquidity_add', 'sig', $2, $3, 1, 0)",
    )
    .bind(address)
    .bind(reserve_a)
    .bind(reserve_b)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutations: the view's sum strict again (a side without a price erasing its pool), and the
/// unvalued pools counted as priced or not at all.
#[sqlx::test]
async fn a_side_without_a_price_counts_zero_and_a_pool_without_any_its_own_count(pool: PgPool) {
    let both = setup_pool(&pool, 1, Some("180.0")).await;
    price_mint(&pool, &mints_of(1).0, "2.5").await;
    let one = setup_pool(&pool, 3, Some("180.0")).await;
    let none = setup_pool(&pool, 2, None).await;
    // 3 A × $2.5 + 7 B × $180; 2 B × $180, A unlisted; nothing priced.
    hold(&pool, &both, 3_000_000, 7_000_000_000).await;
    hold(&pool, &one, 5_000_000, 2_000_000_000).await;
    hold(&pool, &none, 1, 1).await;

    let now = now();
    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    let cycle = cycle_from(now, hours_before(now, 0));
    repository.write_cycle(&cycle).await.unwrap();

    let measured: (DateTime<Utc>, Option<BigDecimal>, i64, i64, DateTime<Utc>) = sqlx::query_as(
        "SELECT bucket, tvl_usd, pools_priced, pools_unvalued, measured_at
           FROM analytic_protocol_tvl WHERE protocol = 'meteora_damm_v2'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        measured,
        (cycle.tvl_hour.unwrap(), Some(dec("1627.5")), 2, 1, cycle.at)
    );
}

/// The discovered part of each window, as `(period, tvl_usd, pools)` in window order.
async fn discovered(pool: &PgPool, bucket: DateTime<Utc>) -> Vec<(String, BigDecimal, i64)> {
    sqlx::query_as(
        "SELECT period, tvl_usd, pools FROM analytic_protocol_tvl_discovered
          WHERE protocol = 'meteora_damm_v2' AND bucket = $1
          ORDER BY CASE period WHEN '24h' THEN 1 WHEN '7d' THEN 2 ELSE 3 END",
    )
    .bind(bucket)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// A pool priced on B at $180 holding `b` B, first seen at `seen`.
async fn seed_pool_seen_at(pool: &PgPool, seed: u8, b: i64, seen: DateTime<Utc>) -> String {
    let address = setup_pool(pool, seed, Some("180.0")).await;
    hold(pool, &address, 1, b * 1_000_000_000).await;
    sqlx::query("UPDATE pools SET first_seen_at = $2 WHERE pool_address = $1")
        .bind(&address)
        .bind(seen)
        .execute(pool)
        .await
        .unwrap();
    address
}

/// Mutations: `first_seen_at` ignored, or compared with `>` (the pool seen at the very start
/// of the 24 hours drops out); the birth ignored (the pool born in the window counts).
#[sqlx::test]
async fn a_pool_discovered_in_a_window_counts_in_its_part_and_one_born_there_does_not(
    pool: PgPool,
) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 0));
    let hour = cycle.tvl_hour.unwrap();
    let start_of_day = hour + Duration::hours(1) - Duration::hours(24);
    seed_pool_seen_at(&pool, 4, 1, hour - Duration::hours(2)).await;
    seed_pool_seen_at(&pool, 8, 16, start_of_day).await;
    let born = seed_pool_seen_at(&pool, 5, 2, hour - Duration::hours(2)).await;
    sqlx::query(
        "INSERT INTO meteora_damm_v2_initialize_pool_events
           (pool_address, signature, token_a_mint, token_b_mint, creator, payer, alpha_vault,
            sqrt_min_price, sqrt_max_price, sqrt_price, liquidity,
            activation_type, activation_point, collect_fee_mode, pool_type,
            token_a_flag, token_b_flag, token_a_amount, token_b_amount,
            total_amount_a, total_amount_b, pool_fees_raw, timestamp, slot, event_index)
         VALUES ($1, $1, 'a', 'b', 'c', 'p', 'v', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                 '\\x00', $2, 0, 0)",
    )
    .bind(&born)
    .bind(hour - Duration::hours(2))
    .execute(&pool)
    .await
    .unwrap();
    seed_pool_seen_at(&pool, 6, 4, hour - Duration::hours(48)).await;
    seed_pool_seen_at(&pool, 7, 8, hour - Duration::days(40)).await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    assert_eq!(
        discovered(&pool, hour).await,
        vec![
            ("24h".to_owned(), dec("3060"), 2),
            ("7d".to_owned(), dec("3780"), 3),
            ("30d".to_owned(), dec("3780"), 3),
        ]
    );
}

/// Mutation: the window started at `from`, not at the start point's last measure: the pool
/// first seen between the two is in neither TVL, and passes for a deposit.
#[sqlx::test]
async fn a_pool_first_seen_after_the_start_measure_counts_as_discovered(pool: PgPool) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 0));
    let hour = cycle.tvl_hour.unwrap();
    let from = hour + Duration::hours(1) - Duration::hours(24);
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl
           (protocol, bucket, tvl_usd, pools_priced, pools_unvalued, measured_at)
         VALUES ('meteora_damm_v2', $1, 720, 1, 0, $2)",
    )
    .bind(hour - Duration::hours(24))
    .bind(from - Duration::minutes(8))
    .execute(&pool)
    .await
    .unwrap();
    seed_pool_seen_at(&pool, 6, 4, from - Duration::minutes(10)).await;
    seed_pool_seen_at(&pool, 4, 1, from - Duration::minutes(3)).await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    assert_eq!(
        discovered(&pool, hour).await[0],
        ("24h".to_owned(), dec("180"), 1)
    );
}

/// Mutation: the pools counted from `pools`, one without a current state counted beside
/// those of the TVL.
#[sqlx::test]
async fn a_pool_without_a_current_state_is_not_counted_discovered(pool: PgPool) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 0));
    setup_pool(&pool, 9, Some("180.0")).await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    assert_eq!(
        discovered(&pool, cycle.tvl_hour.unwrap()).await[0],
        ("24h".to_owned(), dec("0"), 0)
    );
}

/// Mutation: the windows joined to the pools without `LEFT`, a window without one unwritten.
#[sqlx::test]
async fn a_window_without_a_discovered_pool_writes_zero(pool: PgPool) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 0));
    let hour = cycle.tvl_hour.unwrap();
    seed_pool_seen_at(&pool, 7, 8, hour - Duration::days(40)).await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    assert_eq!(
        discovered(&pool, hour).await,
        ["24h", "7d", "30d"]
            .map(|period| (period.to_owned(), dec("0"), 0))
            .to_vec()
    );
}

/// Mutation: the `generate_series` of either writer dropped — an hour without a swap or without a
/// creation unwritten; or `Protocol::all()` for the measured protocols — DLMM written at 0.
#[sqlx::test]
async fn an_hour_without_a_fact_is_written_zero_for_the_measured_protocol_only(pool: PgPool) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 3));

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    let hours = (cycle.through - cycle.from).num_hours() + 1;
    let swaps: Vec<(String, i64, BigDecimal)> = sqlx::query_as(
        "SELECT protocol, swap_count, volume_usd FROM analytic_protocol_hourly ORDER BY bucket",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let births: Vec<(String, i64)> = sqlx::query_as(
        "SELECT protocol, births FROM analytic_protocol_births_hourly ORDER BY bucket",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(swaps.len() as i64, hours, "{swaps:?}");
    assert!(
        swaps
            .iter()
            .all(|(p, n, v)| p == "meteora_damm_v2" && *n == 0 && *v == dec("0"))
    );
    assert_eq!(births.len() as i64, hours, "{births:?}");
    assert!(
        births
            .iter()
            .all(|(p, n)| p == "meteora_damm_v2" && *n == 0)
    );
}

/// Mutation: an hour without a fact zeroed by a `COALESCE` on the sums — an hour whose pools are
/// all unvalued written at 0 instead of `null`.
#[sqlx::test]
async fn a_protocol_hour_of_unvalued_pools_stays_unvalued(pool: PgPool) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 3));
    let hour = hours_before(now, 2);
    sqlx::query(
        "INSERT INTO analytic_pool_5min (pool_address, bucket, protocol, swap_count, valued)
         VALUES ($1, $2, 'meteora_damm_v2', 1, false)",
    )
    .bind(pk(1).to_string())
    .bind(hour + Duration::minutes(5))
    .execute(&pool)
    .await
    .unwrap();

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    let (volume, pools, valued): (Option<BigDecimal>, i64, i64) = sqlx::query_as(
        "SELECT volume_usd, active_pools, active_pools_valued
           FROM analytic_protocol_hourly WHERE bucket = $1",
    )
    .bind(hour)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((volume, pools, valued), (None, 1, 0));
}

/// Against `yog_analytic` made a member of `pg_read_all_data`, which reads
/// every role's password hash.
#[sqlx::test]
async fn yog_analytic_cannot_read_the_password_hashes(pool: PgPool) {
    let analytic = setup_roles_and_connect_as(&pool, "yog_analytic").await;

    assert_reads_no_password_hash(&mut analytic.acquire().await.unwrap()).await;
}

#[sqlx::test]
async fn yog_analytic_cannot_write_signals(pool: PgPool) {
    let analytic = setup_roles_and_connect_as(&pool, "yog_analytic").await;

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

/// `n` rows of the position event `sql` inserts, at `at`, each with its own signature.
async fn insert_position_events(pool: &PgPool, sql: &'static str, n: usize, at: DateTime<Utc>) {
    for i in 0..n {
        sqlx::query(sql)
            .bind(format!("{sql:.40}-{i}"))
            .bind(at)
            .execute(pool)
            .await
            .unwrap();
    }
}

/// Mutation: one kind counted in another's column, or the hours without a change unwritten —
/// the `generate_series` dropped.
#[sqlx::test]
async fn each_kind_of_position_change_is_counted_in_its_hour_and_an_hour_without_any_is_zero(
    pool: PgPool,
) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 3));
    let at = cycle.from + Duration::minutes(10);
    for (sql, n) in [
        (
            "INSERT INTO meteora_damm_v2_create_position_events
               (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
                event_index)
             VALUES ('x', $1, 'x', 'x', 'x', $2, 0, 0)",
            5,
        ),
        (
            "INSERT INTO meteora_damm_v2_lock_position_events
               (pool_address, signature, position, owner, vesting, cliff_point, period_frequency,
                cliff_unlock_liquidity, liquidity_per_period, number_of_period, timestamp, slot,
                event_index)
             VALUES ('x', $1, 'x', 'x', 'x', 0, 0, 0, 0, 0, $2, 0, 0)",
            4,
        ),
        (
            "INSERT INTO meteora_damm_v2_permanent_lock_position_events
               (pool_address, signature, position, lock_liquidity_amount,
                total_permanent_locked_liquidity, timestamp, slot, event_index)
             VALUES ('x', $1, 'x', 0, 0, $2, 0, 0)",
            3,
        ),
        (
            "INSERT INTO meteora_damm_v2_split_position_events (pool_address, signature, first_owner, second_owner, first_position, second_position, current_sqrt_price, split_permanent_locked_liquidity, split_unlocked_liquidity, split_vested_liquidity, split_fee_a, split_fee_b, split_reward_0, split_reward_1, first_unlocked_liquidity, first_permanent_locked_liquidity, first_vested_liquidity, first_fee_a, first_fee_b, first_reward_0, first_reward_1, second_unlocked_liquidity, second_permanent_locked_liquidity, second_vested_liquidity, second_fee_a, second_fee_b, second_reward_0, second_reward_1, num_unlocked_liquidity, num_permanent_locked_liquidity, num_fee_a, num_fee_b, num_reward_0, num_reward_1, num_inner_vesting_liquidity, timestamp, slot, event_index) VALUES ('x', $1, 'x', 'x', 'x', 'x', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, $2, 0, 0)",
            2,
        ),
        (
            "INSERT INTO meteora_damm_v2_close_position_events
               (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
                event_index)
             VALUES ('x', $1, 'x', 'x', 'x', $2, 0, 0)",
            1,
        ),
    ] {
        insert_position_events(&pool, sql, n, at).await;
    }

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    let hours: Vec<(DateTime<Utc>, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT bucket, opened, vesting_locks, permanent_locks, splits, closed
           FROM analytic_protocol_position_changes_hourly ORDER BY bucket",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        hours.len() as i64,
        (cycle.through - cycle.from).num_hours() + 1,
        "{hours:?}"
    );
    assert_eq!(hours[0], (cycle.from, 5, 4, 3, 2, 1));
    assert!(
        hours[1..]
            .iter()
            .all(|h| (h.1, h.2, h.3, h.4, h.5) == (0, 0, 0, 0, 0))
    );
}

/// Mutation: the cycle's hours alone rewritten — an hour written partly before it, and frozen
/// since, kept partial, and the hours after it never written.
#[sqlx::test]
async fn a_cycle_rewrites_from_the_last_hour_already_written(pool: PgPool) {
    let now = now();
    let cycle = cycle_from(now, hours_before(now, 1));
    let partial = cycle.from - Duration::hours(3);
    sqlx::query(
        "INSERT INTO analytic_protocol_position_changes_hourly
           (protocol, bucket, opened, vesting_locks, permanent_locks, splits, closed)
         VALUES ('meteora_damm_v2', $1, 1, 0, 0, 0, 0)",
    )
    .bind(partial)
    .execute(&pool)
    .await
    .unwrap();
    insert_position_events(
        &pool,
        "INSERT INTO meteora_damm_v2_create_position_events
           (pool_address, signature, owner, position, position_nft_mint, timestamp, slot,
            event_index)
         VALUES ('x', $1, 'x', 'x', 'x', $2, 0, 0)",
        3,
        partial + Duration::minutes(50),
    )
    .await;

    let repository =
        PgAnalyticRepository::new(setup_roles_and_connect_as(&pool, "yog_analytic").await);
    repository.write_cycle(&cycle).await.unwrap();

    let opened: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
        "SELECT bucket, opened FROM analytic_protocol_position_changes_hourly ORDER BY bucket",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(opened[0], (partial, 3));
    assert_eq!(
        opened.len() as i64,
        (cycle.through - partial).num_hours() + 1,
        "{opened:?}"
    );
}
