//! `PgDiscoveredTvlRepository` under `yog_api`: the discovered part of the TVL readings, seeded
//! here.

use chrono::Duration;
use sqlx::PgPool;
use yog_core::domain::{DiscoveredTvlLookup, Window};
use yog_persistence::PgDiscoveredTvlRepository;

use super::helpers::{as_of, setup_roles_and_connect_as};

async fn repository(pool: &PgPool) -> PgDiscoveredTvlRepository {
    PgDiscoveredTvlRepository::new(setup_roles_and_connect_as(pool, "yog_api").await)
}

/// Mutation: the window's `period` not filtered, or the reading taken after the hour — the
/// 7-day part, or the open hour's, read.
#[sqlx::test(migrations = "./migrations")]
async fn the_discoveries_are_those_of_the_window_at_the_last_reading(pool: PgPool) {
    let last = as_of() - Duration::hours(1);
    for (period, bucket, pools) in [
        ("24h", last - Duration::hours(1), 3),
        ("7d", last, 40),
        ("24h", last + Duration::hours(1), 99),
    ] {
        sqlx::query(
            "INSERT INTO analytic_protocol_tvl_discovered (protocol, period, bucket, tvl_usd, pools)
             VALUES ('meteora_damm_v2', $1, $2, 7, $3)",
        )
        .bind(period)
        .bind(bucket)
        .bind(pools)
        .execute(&pool)
        .await
        .unwrap();
    }
    let repository = repository(&pool).await;

    let found = DiscoveredTvlLookup::list_totals(&repository, last, Window::Day, None)
        .await
        .unwrap();

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].pools, 3);
    assert_eq!(found[0].at, last);
    let week = DiscoveredTvlLookup::list_totals(&repository, last, Window::Week, None)
        .await
        .unwrap();
    assert_eq!(week[0].pools, 40);
}
