//! `PgProtocolTvlRepository` under `yog_api`: the readings of one protocol and their change over
//! a window, seeded here.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::domain::{Protocol, ProtocolTvlLookup, TvlReading, Window};
use yog_persistence::PgProtocolTvlRepository;

use super::helpers::{DAMM, DLMM, as_of, setup_roles_and_connect_as};

async fn repository(pool: &PgPool) -> PgProtocolTvlRepository {
    PgProtocolTvlRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
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
    let hourly = ProtocolTvlLookup::list_hourly(
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

/// Mutations: either end excluded (the variation loses its start, or the series stops an hour
/// short), the hours out of order, or DLMM left out of the series of every protocol.
#[sqlx::test(migrations = "./migrations")]
async fn the_tvl_series_holds_both_ends_and_every_protocol(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    for (protocol, base) in [(DAMM, 0), (DLMM, 100)] {
        for offset in -3..=1 {
            insert_tvl(
                &pool,
                protocol,
                last + Duration::hours(offset),
                base + offset,
            )
            .await;
        }
    }
    let repository = repository(&pool).await;

    let series = ProtocolTvlLookup::list_hourly(&repository, last - Duration::hours(2), last, None)
        .await
        .unwrap();

    let read: Vec<_> = series
        .iter()
        .map(|t| (t.protocol.as_str(), t.at, t.tvl_usd))
        .collect();
    let expected: Vec<_> = (-2..=0)
        .flat_map(|offset| {
            [(DAMM, 0), (DLMM, 100)].map(|(protocol, base)| {
                let at = last + Duration::hours(offset + 1);
                (protocol, at, Some(Decimal::from(base + offset)))
            })
        })
        .collect();
    assert_eq!(read, expected);
}

async fn insert_discovered(pool: &PgPool, period: &str, bucket: DateTime<Utc>, tvl: i64) {
    sqlx::query(
        "INSERT INTO analytic_protocol_tvl_discovered (protocol, period, bucket, tvl_usd, pools)
         VALUES ($1, $2, $3, $4, 1)",
    )
    .bind(DAMM)
    .bind(period)
    .bind(bucket)
    .bind(tvl)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutations: the start taken as the series' first reading, or the discovered part as the last
/// one, another window's, or one at another reading than the end's; or a protocol without a
/// start left out, its addition then unable to see the term it misses.
#[sqlx::test(migrations = "./migrations")]
async fn the_change_pairs_the_reading_a_window_before_and_the_part_at_the_end(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    let end = last - Duration::hours(1);
    insert_tvl(&pool, DAMM, end - Duration::hours(30), 500).await;
    insert_tvl(&pool, DAMM, end - Duration::hours(24), 1_000).await;
    insert_tvl(&pool, DAMM, end - Duration::hours(23), 990).await;
    insert_tvl(&pool, DAMM, end, 1_300).await;
    insert_discovered(&pool, "24h", end, 200).await;
    insert_discovered(&pool, "24h", last, 999).await;
    insert_discovered(&pool, "7d", end, 77).await;
    insert_tvl(&pool, DLMM, last, 5).await;
    let repository = repository(&pool).await;

    let changes = repository
        .list_changes(last, Window::Day, None)
        .await
        .unwrap();

    let read: Vec<_> = changes
        .iter()
        .map(|c| {
            (
                c.protocol.as_str(),
                c.end.clone(),
                c.start.clone(),
                c.discovered_tvl_usd,
            )
        })
        .collect();
    let reading = |at: DateTime<Utc>, tvl: i64| TvlReading {
        at: at + Duration::hours(1),
        tvl_usd: Some(Decimal::from(tvl)),
    };
    assert_eq!(
        read,
        [
            (
                DAMM,
                reading(end, 1_300),
                Some(reading(end - Duration::hours(24), 1_000)),
                Some(Decimal::from(200))
            ),
            (DLMM, reading(last, 5), None, None),
        ]
    );
}

/// Mutation: the protocol filter dropped — DLMM's change read under DAMM v2.
#[sqlx::test(migrations = "./migrations")]
async fn the_change_of_one_protocol_is_its_own(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    insert_tvl(&pool, DAMM, last, 1_300).await;
    insert_tvl(&pool, DLMM, last, 5).await;
    let repository = repository(&pool).await;

    let changes = repository
        .list_changes(last, Window::Day, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].protocol, Protocol::MeteoraDammV2);
}
