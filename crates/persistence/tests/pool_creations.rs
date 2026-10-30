//! `PgPoolCreationsRepository` under `yog_api`: the births of one protocol, seeded here.

use sqlx::PgPool;
use yog_core::domain::{PoolCreationsLookup, Protocol};
use yog_persistence::PgPoolCreationsRepository;

use super::helpers::{DAMM, DLMM, as_of, period, setup_roles_and_connect_as, write_frozen_hours};

async fn repository(pool: &PgPool) -> PgPoolCreationsRepository {
    PgPoolCreationsRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

/// Mutation: the protocol filter of the creations dropped — DLMM's counted.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_creations_count_only_it(pool: PgPool) {
    for (protocol, births) in [(DAMM, 2), (DLMM, 50)] {
        sqlx::query(
            "INSERT INTO analytic_protocol_births_hourly (protocol, bucket, births)
             VALUES ($1, $2 - INTERVAL '1 hour', $3), ($1, $2 - INTERVAL '2 hours', $3)",
        )
        .bind(protocol)
        .bind(as_of())
        .bind(births)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;
    let day = period(24);

    let totals = PoolCreationsLookup::list_totals(&repository, &day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let hourly = PoolCreationsLookup::list_hourly(&repository, &day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let all = PoolCreationsLookup::list_totals(&repository, &day, None)
        .await
        .unwrap();

    assert_eq!(totals.len(), 1);
    assert_eq!(totals[0].pools, 4);
    assert_eq!(hourly.len(), 2);
    assert_eq!(all.len(), 2);
}

/// Mutation: yog-analytic's births written from the creations alone — an hour without one
/// unwritten, its window without totals.
#[sqlx::test(migrations = "./migrations")]
async fn a_protocols_hour_without_a_creation_reads_zero(pool: PgPool) {
    write_frozen_hours(&pool, 24).await;
    let repository = repository(&pool).await;
    let day = period(24);

    let totals = PoolCreationsLookup::list_totals(&repository, &day, None)
        .await
        .unwrap();
    let hourly = PoolCreationsLookup::list_hourly(&repository, &day, None)
        .await
        .unwrap();
    let daily =
        PoolCreationsLookup::list_buckets(&repository, &day, chrono::Duration::days(1), None)
            .await
            .unwrap();

    assert_eq!(
        totals.len(),
        1,
        "DAMM v2 alone, DLMM unmeasured: {totals:?}"
    );
    assert_eq!(totals[0].pools, 0);
    assert_eq!(hourly.len(), 24);
    assert!(hourly.iter().all(|b| b.value.pools == 0));
    assert_eq!(daily.len(), 1);
    let slices =
        PoolCreationsLookup::list_buckets(&repository, &day, chrono::Duration::hours(6), None)
            .await
            .unwrap();
    assert_eq!(slices.len(), 4);
}
