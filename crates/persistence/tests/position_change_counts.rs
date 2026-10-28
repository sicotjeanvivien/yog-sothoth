//! `PgPositionChangeCountsRepository` under `yog_api`: the position changes of a protocol over a
//! frozen period, from the hours yog-analytic wrote, seeded here.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use yog_core::domain::{PositionChangeCounts, PositionChangeCountsLookup, Protocol};
use yog_persistence::PgPositionChangeCountsRepository;

use super::helpers::{DAMM, DLMM, as_of, period, setup_roles_and_connect_as};

async fn insert_hour(pool: &PgPool, protocol: &str, bucket: DateTime<Utc>, n: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_position_changes_hourly
           (protocol, bucket, opened, vesting_locks, permanent_locks, splits, closed)
         VALUES ($1, $2, $3, $3 * 2, $3 * 3, $3 * 4, $3 * 5)",
    )
    .bind(protocol)
    .bind(bucket)
    .bind(n)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: a bound of the period dropped, or the protocol filter — the hour before the window,
/// the open hour or DLMM's hour summed in.
#[sqlx::test(migrations = "./migrations")]
async fn the_totals_sum_the_protocols_hours_of_the_period(pool: PgPool) {
    insert_hour(&pool, DAMM, as_of() - Duration::hours(11), 100).await;
    insert_hour(&pool, DAMM, as_of() - Duration::hours(10), 1).await;
    insert_hour(&pool, DAMM, as_of() - Duration::hours(1), 2).await;
    insert_hour(&pool, DAMM, as_of(), 100).await;
    insert_hour(&pool, DLMM, as_of() - Duration::hours(1), 100).await;
    let repository =
        PgPositionChangeCountsRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let totals = repository
        .find_totals(&period(10), Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(
        totals,
        Some(PositionChangeCounts {
            opened: 3,
            vesting_locks: 6,
            permanent_locks: 9,
            splits: 12,
            closed: 15,
        })
    );
}
