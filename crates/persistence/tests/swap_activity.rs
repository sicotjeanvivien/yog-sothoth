//! `PgSwapActivityRepository` under `yog_api`: the swaps read by scope, from the hours as yog-analytic
//! leaves them, seeded here.

use chrono::Duration;
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::domain::{FrozenPeriod, Protocol, SwapActivityLookup, Window};
use yog_persistence::PgSwapActivityRepository;

use super::helpers::{DAMM, DLMM, as_of, period, pk, setup_roles_and_connect_as};

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

/// One pool-hour per hour of `period(hours)`, `volume` dollars on side A; the last one unvalued.
async fn seed_pool_hours(pool: &PgPool, address: &str, hours: i32, volume: i64) {
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
