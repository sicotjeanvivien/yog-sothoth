//! The Overview's reads under `yog_api` that are not a measure — the frozen hours,
//! `PgTokenActivityRepository` and `PgPoolRepository::counts_by_protocol` — from the tables
//! as their writers leave them, seeded here directly.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use yog_core::domain::{
    FrozenPeriod, PoolCatalog, Protocol, ProtocolActivityLookup, TokenActivityCursor,
    TokenActivityFeed, Window,
};
use yog_persistence::{PgPoolRepository, PgProtocolActivityRepository, PgTokenActivityRepository};

use super::helpers::{pk, setup_roles_and_connect_as};

/// The last frozen hour of every test: 12:00, so a day of 24 hours never starts at midnight.
fn last() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap()
}

/// `hours` hours written through [`last`], frozen, plus the open hour after it.
async fn seed_hours(pool: &PgPool, hours: i32) {
    sqlx::query(
        "INSERT INTO analytic_hours (bucket, frozen_at)
         SELECT h, CASE WHEN h <= $1 THEN $1 + INTERVAL '2 hours' END
           FROM generate_series($1 - make_interval(hours => $2 - 1),
                                $1 + INTERVAL '1 hour', INTERVAL '1 hour') AS h",
    )
    .bind(last())
    .bind(hours)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: `frozen_hours` read without `frozen_at` — the open hour counted as frozen.
#[sqlx::test(migrations = "./migrations")]
async fn no_read_counts_the_open_hour(pool: PgPool) {
    seed_hours(&pool, 30).await;
    let repository =
        PgProtocolActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let hours = repository.frozen_hours().await.unwrap().unwrap();

    assert_eq!(hours.last_frozen, last());
    assert_eq!(
        FrozenPeriod::of(hours, Window::Day, None).as_of,
        last() + Duration::hours(1)
    );
}

async fn insert_token_period(pool: &PgPool, mint: u8, volume: i64, at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO analytic_token_periods
           (period, mint, bucket, volume_usd, pools, pool_hours, pool_hours_valued)
         VALUES ('24h', $1, $2, $3, 1, 1, 1)",
    )
    .bind(pk(mint).to_string())
    .bind(at)
    .bind(volume)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutations: the cursor's tie on volume broken the other way (a token served
/// twice, another skipped), or the identity left unjoined.
#[sqlx::test(migrations = "./migrations")]
async fn the_tokens_page_by_volume_then_mint_with_their_identity(pool: PgPool) {
    for (mint, volume) in [(1, 30), (2, 20), (3, 20), (4, 5)] {
        insert_token_period(&pool, mint, volume, last()).await;
    }
    sqlx::query(
        "INSERT INTO token_metadata (mint, symbol, decimals, fetched_at, last_refresh_at)
         VALUES ($1, 'ONE', 9, now(), now())",
    )
    .bind(pk(1).to_string())
    .execute(&pool)
    .await
    .unwrap();
    let feed = PgTokenActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let first = feed.list(Window::Day, None, 2).await.unwrap().unwrap();
    let cursor = first.page.items.last().map(|t| TokenActivityCursor {
        last_hour: first.last_hour,
        volume_usd: t.volume_usd,
        mint: t.mint,
    });
    let second = feed.list(Window::Day, cursor, 2).await.unwrap().unwrap();
    let mints = |items: &[yog_core::domain::TokenActivity]| -> Vec<_> {
        items.iter().map(|t| t.mint).collect()
    };

    let (low, high) = if pk(2) < pk(3) { (2, 3) } else { (3, 2) };
    assert_eq!(first.last_hour, last());
    assert_eq!(mints(&first.page.items), [pk(1), pk(low)]);
    assert_eq!(mints(&second.page.items), [pk(high), pk(4)]);
    assert!(!first.page.is_last && second.page.is_last && first.page.prev_cursor.is_none());
    let symbol = first.page.items[0]
        .metadata
        .as_ref()
        .and_then(|m| m.symbol.as_deref());
    assert_eq!(symbol, Some("ONE"));
    assert!(first.page.items[1].metadata.is_none());
}

/// Mutation: the hour taken from the rows — an empty page past the last token
/// would lose the hour of its ranking, and a ranking never written read as one.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_past_the_last_token_still_names_its_ranking(pool: PgPool) {
    insert_token_period(&pool, 1, 30, last()).await;
    let feed = PgTokenActivityRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let past = TokenActivityCursor {
        last_hour: last(),
        volume_usd: Decimal::from(1),
        mint: pk(9),
    };
    let page = feed
        .list(Window::Day, Some(past), 10)
        .await
        .unwrap()
        .unwrap();
    let week = feed.list(Window::Week, None, 10).await.unwrap();

    assert_eq!(page.last_hour, last());
    assert!(page.page.items.is_empty() && page.page.is_last);
    assert!(week.is_none());
}

async fn insert_seen_pool(pool: &PgPool, seed: u8, protocol: Protocol, mints: Option<(u8, u8)>) {
    sqlx::query(
        "INSERT INTO pools (pool_address, protocol, token_a_mint, token_b_mint)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(pk(seed).to_string())
    .bind(protocol.as_str())
    .bind(mints.map(|(a, _)| pk(a).to_string()))
    .bind(mints.map(|(_, b)| pk(b).to_string()))
    .execute(pool)
    .await
    .unwrap();
}

/// Mutations: the pools counted on their mints (a pool the context has not
/// resolved yet vanishes), DAMM v2 alone (DLMM vanishes), or a 0 row for a
/// protocol without a pool.
#[sqlx::test(migrations = "./migrations")]
async fn every_pool_seen_counts_under_its_protocol(pool: PgPool) {
    let catalog = PgPoolRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    assert_eq!(catalog.counts_by_protocol().await.unwrap(), []);
    insert_seen_pool(&pool, 1, Protocol::MeteoraDammV2, Some((101, 102))).await;
    insert_seen_pool(&pool, 2, Protocol::MeteoraDammV2, None).await;
    insert_seen_pool(&pool, 3, Protocol::MeteoraDlmm, Some((101, 103))).await;

    let mut counts: Vec<_> = catalog
        .counts_by_protocol()
        .await
        .unwrap()
        .into_iter()
        .map(|p| (p.protocol.as_str(), p.pools_seen))
        .collect();
    counts.sort();

    let expected = [
        (Protocol::MeteoraDammV2.as_str(), 2),
        (Protocol::MeteoraDlmm.as_str(), 1),
    ];
    assert_eq!(counts, expected);
}
