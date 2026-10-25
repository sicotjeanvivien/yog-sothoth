//! `PgTvlRepository` under `yog_api`: the readings of one protocol, seeded here.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::domain::{Protocol, TvlLookup};
use yog_persistence::PgTvlRepository;

use super::helpers::{DAMM, DLMM, as_of, setup_roles_and_connect_as};

async fn repository(pool: &PgPool) -> PgTvlRepository {
    PgTvlRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

async fn insert_tvl(pool: &PgPool, protocol: &str, bucket: DateTime<Utc>, tvl: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl
           (protocol, bucket, tvl_usd, pools_priced, pools_unvalued, measured_at)
         VALUES ($1, $2, $3, 1, 0, $2)",
    )
    .bind(protocol)
    .bind(bucket)
    .bind(tvl)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the latest reading taken after the hour asked, or the protocol filter dropped —
/// the reading of the open hour, or DLMM's, read.
#[sqlx::test(migrations = "./migrations")]
async fn the_tvl_is_the_last_reading_at_or_before_the_hour_with_its_instant(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    insert_tvl(&pool, DAMM, last - Duration::hours(3), 100).await;
    insert_tvl(&pool, DAMM, last - Duration::hours(1), 110).await;
    insert_tvl(&pool, DAMM, last + Duration::hours(1), 999).await;
    insert_tvl(&pool, DLMM, last, 5).await;
    let repository = repository(&pool).await;

    let latest = repository
        .list_latest(last, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    let hourly = TvlLookup::list_hourly(
        &repository,
        last - Duration::hours(3),
        last,
        Some(Protocol::MeteoraDammV2),
    )
    .await
    .unwrap();

    assert_eq!(latest.len(), 1);
    assert_eq!(latest[0].tvl_usd, Some(Decimal::from(110)));
    assert_eq!(latest[0].at, last);
    assert_eq!(hourly.len(), 2);
}
