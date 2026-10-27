//! `PgSwapActivityRepository` under `yog_api`: the swaps read by scope, from the hours as yog-analytic
//! leaves them, seeded here.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::domain::{FrozenHours, FrozenPeriod, Protocol, SwapActivityLookup, Window};
use yog_persistence::PgSwapActivityRepository;

use super::helpers::{
    DAMM, DLMM, as_of, period, pk, setup_roles_and_connect_as, write_frozen_hours,
};

async fn repository(pool: &PgPool) -> PgSwapActivityRepository {
    PgSwapActivityRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

/// One protocol-hour per hour of `period(hours)`: `volume` dollars and one swap each.
async fn seed_protocol_hours(pool: &PgPool, protocol: &str, hours: i32, volume: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_hourly
           (protocol, bucket, volume_usd, fees_usd, lp_fees_usd, protocol_fees_usd,
            referral_fees_usd, swap_count, active_pools, active_pools_valued)
         SELECT $1, h, $4, 0, 0, 0, 0, 1, 1, 1
           FROM generate_series($2 - make_interval(hours => $3), $2 - INTERVAL '1 hour',
                                INTERVAL '1 hour') AS h",
    )
    .bind(protocol)
    .bind(as_of())
    .bind(hours)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

/// The pool, first seen at `first_seen`, and the frozen hours of `period(hours)`.
async fn know_pool(pool: &PgPool, address: &str, first_seen: DateTime<Utc>, hours: i32) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, first_seen_at, last_seen_at)
         VALUES ($1, 'meteora_damm_v2', $2, $2) ON CONFLICT DO NOTHING",
    )
    .bind(address)
    .bind(first_seen)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO analytic_hours (bucket, frozen_at)
         SELECT h, $1 FROM generate_series($1 - make_interval(hours => $2), $1 - INTERVAL '1 hour',
                                           INTERVAL '1 hour') AS h
         ON CONFLICT DO NOTHING",
    )
    .bind(as_of())
    .bind(hours)
    .execute(pool)
    .await
    .unwrap();
}

/// One pool-hour at `at`: two swaps, `volume` dollars on side A, or unvalued when `None`.
async fn seed_pool_hour(pool: &PgPool, address: &str, at: DateTime<Utc>, volume: Option<i64>) {
    sqlx::query(
        "INSERT INTO analytic_pool_hourly
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd,
            volume_in_b_usd, fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd)
         VALUES ($1, $2, 'meteora_damm_v2', 2, $3 IS NOT NULL, $3, $4, 0, 0, 0, 0)",
    )
    .bind(address)
    .bind(at)
    .bind(volume)
    .bind(volume.map(|_| 0i64))
    .execute(pool)
    .await
    .unwrap();
}

/// One pool-hour per hour of `period(hours)`, `volume` dollars on side A; the last one unvalued.
async fn seed_pool_hours(pool: &PgPool, address: &str, hours: i32, volume: i64) {
    know_pool(
        pool,
        address,
        as_of() - Duration::hours(hours.into()),
        hours,
    )
    .await;
    sqlx::query(
        "INSERT INTO analytic_pool_hourly
           (pool_address, bucket, protocol, swap_count, valued, volume_in_a_usd,
            volume_in_b_usd, fees_usd, lp_fees_usd, protocol_fees_usd, referral_fees_usd)
         SELECT $1, h, 'meteora_damm_v2', 2, h < $2 - INTERVAL '1 hour',
                CASE WHEN h < $2 - INTERVAL '1 hour' THEN $4 END,
                CASE WHEN h < $2 - INTERVAL '1 hour' THEN 0 END, 0, 0, 0, 0
           FROM generate_series($2 - make_interval(hours => $3), $2 - INTERVAL '1 hour',
                                INTERVAL '1 hour') AS h",
    )
    .bind(address)
    .bind(as_of())
    .bind(hours)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the protocol filter of the totals and the series dropped — DLMM's hours counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_swaps_count_only_it(pool: PgPool) {
    seed_protocol_hours(&pool, DAMM, 24, 1).await;
    seed_protocol_hours(&pool, DLMM, 24, 100).await;
    let repository = repository(&pool).await;
    let day = period(24);

    let all = repository
        .find_protocol_totals(&day, Window::Day, None)
        .await
        .unwrap();
    let damm = repository
        .find_protocol_totals(&day, Window::Day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let hourly = repository
        .list_protocol_hourly(&day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let daily = repository
        .list_protocol_daily(&day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert_eq!(all.len(), 2);
    assert_eq!(damm.len(), 1);
    assert_eq!(damm[0].activity.volume_usd, Some(Decimal::from(24)));
    assert_eq!(damm[0].activity.swaps, 24);
    assert_eq!(hourly.len(), 24);
    assert!(hourly.iter().all(|b| b.protocol == Protocol::MeteoraDammV2));
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0].value.volume_usd, Some(Decimal::from(24)));
}

/// Mutation: the pool filter dropped — the other pool's volume counted; or `valued` ignored in
/// the coverage — the unvalued hour counted as valued.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_swaps_count_only_it_and_say_their_coverage(pool: PgPool) {
    let (mine, other) = (pk(1), pk(2));
    seed_pool_hours(&pool, &mine.to_string(), 24, 10).await;
    seed_pool_hours(&pool, &other.to_string(), 24, 1000).await;
    let repository = repository(&pool).await;
    let day = period(24);

    let totals = repository
        .find_pool_totals(&day, mine)
        .await
        .unwrap()
        .unwrap();
    let hourly = repository.list_pool_hourly(&day, mine).await.unwrap();

    assert_eq!(totals.volume_usd, Some(Decimal::from(230)));
    assert_eq!(totals.swaps, 48);
    assert_eq!(totals.pool_hours, 24);
    assert_eq!(totals.pool_hours_valued, 23);
    assert_eq!(hourly.len(), 24);
    assert_eq!(hourly.last().unwrap().activity.pool_hours_valued, 0);
    assert!(
        repository
            .find_pool_totals(&day, pk(3))
            .await
            .unwrap()
            .is_none()
    );
}

/// Mutation: the pool's days cut at midnight, or the `covered_since` bound dropped — a first
/// day the facts do not hold whole is drawn.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_day_is_24_hours_ending_at_as_of_and_held_whole(pool: PgPool) {
    let mine = pk(1);
    seed_pool_hours(&pool, &mine.to_string(), 24 * 2 + 12, 1).await;
    let repository = repository(&pool).await;
    let three_days = FrozenPeriod {
        covered_since: as_of() - Duration::hours(24 * 2 + 12),
        ..period(24 * 3)
    };

    let daily = repository.list_pool_daily(&three_days, mine).await.unwrap();

    assert_eq!(daily.len(), 2);
    assert_eq!(daily[0].from, as_of() - Duration::hours(48));
    assert_eq!(daily[1].activity.pool_hours, 24);
}

/// Mutation: a protocol's days cut at midnight, or the `covered_since` bound dropped — a first
/// day the facts do not hold whole is drawn.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_day_is_24_hours_ending_at_as_of_and_held_whole(pool: PgPool) {
    seed_protocol_hours(&pool, DAMM, 24 * 6 + 12, 1).await;
    let repository = repository(&pool).await;
    let week = FrozenPeriod {
        covered_since: as_of() - Duration::hours(24 * 6 + 12),
        ..period(24 * 7)
    };

    let daily = repository.list_protocol_daily(&week, None).await.unwrap();

    assert_eq!(daily.len(), 6);
    assert_eq!(daily[0].from, week.from + Duration::days(1));
    assert!(
        daily
            .iter()
            .all(|d| d.value.volume_usd == Some(Decimal::from(24)))
    );
}

/// Mutation: the distinct pools joined without the period's last hour — the window before
/// reads the pools written for the window now.
#[sqlx::test(migrations = "./migrations")]
async fn the_previous_window_reads_the_pools_written_a_window_earlier(pool: PgPool) {
    seed_protocol_hours(&pool, DAMM, 24 * 3, 1).await;
    for (hours_back, pools) in [(1, 10), (25, 7)] {
        sqlx::query(
            "INSERT INTO analytic_protocol_periods (protocol, period, bucket, active_pools)
             VALUES ('meteora_damm_v2', '24h', $1, $2)",
        )
        .bind(as_of() - Duration::hours(hours_back))
        .bind(pools)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;
    let hours = FrozenHours {
        first: as_of() - Duration::hours(24 * 3),
        last_frozen: as_of() - Duration::hours(1),
    };
    let read = |offset| {
        let repository = &repository;
        async move {
            let period = FrozenPeriod::of(hours, Window::Day, offset);
            let rows = repository
                .find_protocol_totals(&period, Window::Day, None)
                .await
                .unwrap();
            rows.iter().map(|row| row.pools).collect::<Vec<_>>()
        }
    };

    assert_eq!(read(None).await, [Some(10)]);
    assert_eq!(read(Some(Window::Day)).await, [Some(7)]);
}

/// Mutation: the pool's hours read from its facts alone — a covered hour without a swap missing,
/// and a pool that never swapped without totals.
#[sqlx::test(migrations = "./migrations")]
async fn a_pools_covered_hour_without_a_swap_is_zero(pool: PgPool) {
    let (busy, quiet) = (pk(1), pk(2));
    know_pool(&pool, &busy.to_string(), as_of() - Duration::hours(48), 24).await;
    know_pool(&pool, &quiet.to_string(), as_of() - Duration::hours(48), 24).await;
    seed_pool_hour(
        &pool,
        &busy.to_string(),
        as_of() - Duration::hours(3),
        Some(10),
    )
    .await;
    let repository = repository(&pool).await;
    let day = period(24);

    let hourly = repository.list_pool_hourly(&day, busy).await.unwrap();
    let daily = repository.list_pool_daily(&day, quiet).await.unwrap();
    let quiet_totals = repository.find_pool_totals(&day, quiet).await.unwrap();

    assert_eq!(hourly.len(), 24, "every covered hour, swapped or not");
    let empty: Vec<_> = hourly.iter().filter(|b| b.activity.swaps == 0).collect();
    assert_eq!(empty.len(), 23);
    assert!(
        empty
            .iter()
            .all(|b| b.activity.pool_hours == 0 && b.activity.volume_usd == Some(Decimal::ZERO))
    );
    assert_eq!(daily.len(), 1);
    assert_eq!(daily[0].activity.swaps, 0);
    let quiet_totals = quiet_totals.expect("a known pool without a swap totals 0, not nothing");
    assert_eq!((quiet_totals.swaps, quiet_totals.pool_hours), (0, 0));
    assert_eq!(quiet_totals.volume_usd, Some(Decimal::ZERO));
}

/// Mutation: the bound of `first_seen_at` dropped — zero buckets drawn before the pool appeared.
#[sqlx::test(migrations = "./migrations")]
async fn a_pool_has_no_bucket_before_the_hour_it_appeared(pool: PgPool) {
    let born = pk(1);
    know_pool(
        &pool,
        &born.to_string(),
        as_of() - Duration::minutes(10 * 60 - 20),
        24,
    )
    .await;
    let repository = repository(&pool).await;

    let hourly = repository
        .list_pool_hourly(&period(24), born)
        .await
        .unwrap();

    assert_eq!(hourly.len(), 10, "from the hour of its first_seen_at on");
    assert_eq!(hourly[0].from, as_of() - Duration::hours(10));
}

/// Mutation: the zeros bounded by `first_seen_at` alone — a fact indexed late, older than the pool's
/// first event, dropped, or the hours between them missing.
#[sqlx::test(migrations = "./migrations")]
async fn a_fact_older_than_first_seen_at_starts_the_pools_hours(pool: PgPool) {
    let late = pk(1);
    know_pool(
        &pool,
        &late.to_string(),
        as_of() - Duration::minutes(5 * 60 - 10),
        24,
    )
    .await;
    seed_pool_hour(
        &pool,
        &late.to_string(),
        as_of() - Duration::hours(12),
        Some(10),
    )
    .await;
    let repository = repository(&pool).await;

    let hourly = repository
        .list_pool_hourly(&period(24), late)
        .await
        .unwrap();

    assert_eq!(hourly.len(), 12, "every hour from the fact at -12 h on");
    assert_eq!(hourly[0].from, as_of() - Duration::hours(12));
    assert_eq!(hourly[0].activity.swaps, 2);
}

/// Mutation: the pool's protocol not checked against the measured ones — a DLMM pool, whose swaps
/// yog-analytic never values, read at 0.
#[sqlx::test(migrations = "./migrations")]
async fn an_unmeasured_protocols_pool_reads_nothing(pool: PgPool) {
    let dlmm = pk(1);
    know_pool(&pool, &dlmm.to_string(), as_of() - Duration::hours(48), 24).await;
    sqlx::query("UPDATE pools SET protocol = 'meteora_dlmm' WHERE pool_address = $1")
        .bind(dlmm.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let repository = repository(&pool).await;
    let day = period(24);

    assert!(
        repository
            .find_pool_totals(&day, dlmm)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        repository
            .list_pool_hourly(&day, dlmm)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        repository
            .list_pool_daily(&day, dlmm)
            .await
            .unwrap()
            .is_empty()
    );
}

/// Mutation: the zero of an hour without a fact applied to the sums too — an unvalued hour's
/// `null` volume read as 0 beside the quiet hours.
#[sqlx::test(migrations = "./migrations")]
async fn an_unvalued_hour_stays_unvalued_beside_hours_without_a_swap(pool: PgPool) {
    let mine = pk(1);
    know_pool(&pool, &mine.to_string(), as_of() - Duration::hours(48), 24).await;
    seed_pool_hour(&pool, &mine.to_string(), as_of() - Duration::hours(3), None).await;
    let repository = repository(&pool).await;
    let day = period(24);

    let totals = repository
        .find_pool_totals(&day, mine)
        .await
        .unwrap()
        .unwrap();
    let daily = repository.list_pool_daily(&day, mine).await.unwrap();

    assert_eq!(totals.volume_usd, None, "{totals:?}");
    assert_eq!(daily[0].activity.volume_usd, None, "{daily:?}");
    assert_eq!((totals.pool_hours, totals.pool_hours_valued), (1, 0));
}

/// Mutation: yog-analytic's protocol hours written from the facts alone — a protocol hour without
/// a swap unwritten; or written for every protocol — DLMM, unmeasured, read at 0.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_hour_without_a_swap_reads_zero(pool: PgPool) {
    write_frozen_hours(&pool, 24).await;
    let repository = repository(&pool).await;
    let day = period(24);

    let totals = repository
        .find_protocol_totals(&day, Window::Day, None)
        .await
        .unwrap();
    let hourly = repository.list_protocol_hourly(&day, None).await.unwrap();
    let daily = repository.list_protocol_daily(&day, None).await.unwrap();

    assert_eq!(
        totals.len(),
        1,
        "DAMM v2 alone, DLMM unmeasured: {totals:?}"
    );
    assert_eq!(totals[0].protocol, Protocol::MeteoraDammV2);
    assert_eq!(totals[0].activity.swaps, 0);
    assert_eq!(totals[0].activity.volume_usd, Some(Decimal::ZERO));
    assert_eq!(hourly.len(), 24);
    assert!(
        hourly
            .iter()
            .all(|b| b.value.swaps == 0 && b.value.pool_hours == 0)
    );
    assert_eq!(daily.len(), 1);
}

/// Mutation: the `covered_since` bound of the protocol series dropped — hours before the first
/// one measured drawn.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_series_starts_at_covered_since(pool: PgPool) {
    write_frozen_hours(&pool, 24).await;
    let repository = repository(&pool).await;
    let half = FrozenPeriod {
        covered_since: as_of() - Duration::hours(12),
        ..period(24)
    };

    let hourly = repository.list_protocol_hourly(&half, None).await.unwrap();

    assert_eq!(hourly.len(), 12);
    assert_eq!(hourly[0].from, half.covered_since);
}
