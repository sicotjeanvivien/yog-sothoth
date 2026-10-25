//! `PgTokenMetadataRepository`'s catalog read under `yog_api`: the tokens newest first, paged
//! without repeat nor gap, each with its latest price.

use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::{Cursor, domain::TokenMetadataLookup};
use yog_persistence::PgTokenMetadataRepository;

use super::helpers::{pk, price_at, setup_roles_and_connect_as};

fn at(minutes: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 20, 12, 0, 0).unwrap() + Duration::minutes(minutes)
}

async fn insert_metadata(pool: &PgPool, mint: Pubkey, fetched_at: DateTime<Utc>) {
    sqlx::query(
        "INSERT INTO token_metadata (mint, decimals, fetched_at, last_refresh_at)
         VALUES ($1, 6, $2, $2)",
    )
    .bind(mint.to_string())
    .bind(fetched_at)
    .execute(pool)
    .await
    .unwrap();
}

/// Mutation: the cursor compared on the instant alone, or with `<=` — a token fetched in the
/// same instant as the last of a page skipped or repeated; or the order ascending.
#[sqlx::test(migrations = "./migrations")]
async fn a_page_follows_without_repeat_nor_gap(pool: PgPool) {
    for (seed, minutes) in [(1, 1), (2, 1), (3, 2), (4, 2), (5, 3)] {
        insert_metadata(&pool, pk(seed), at(minutes)).await;
    }
    let repository =
        PgTokenMetadataRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);
    let expected: Vec<_> = repository
        .list_entries(None, 50)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|e| e.metadata)
        .collect();

    let mut seen = Vec::new();
    let mut cursor = None;
    loop {
        let page = repository.list_entries(cursor.take(), 2).await.unwrap();
        seen.extend(page.items.into_iter().map(|e| e.metadata.mint));
        match page.next_cursor {
            Some(Cursor::Token(next)) => cursor = Some(next),
            Some(other) => panic!("{other:?}"),
            None => break,
        }
    }

    assert_eq!(seen, expected.iter().map(|m| m.mint).collect::<Vec<_>>());
    assert_eq!(seen.len(), 5);
    assert_eq!(seen[0], pk(5));
    assert!(
        expected
            .windows(2)
            .all(|w| w[0].fetched_at >= w[1].fetched_at)
    );
}

/// Mutation: the latest price joined on another token — the unpriced token priced, the
/// priced one bare.
#[sqlx::test(migrations = "./migrations")]
async fn a_token_carries_its_latest_price(pool: PgPool) {
    let (priced, unpriced) = (pk(1), pk(2));
    insert_metadata(&pool, priced, at(1)).await;
    insert_metadata(&pool, unpriced, at(2)).await;
    price_at(&pool, &priced.to_string(), "3", at(10)).await;
    price_at(&pool, &priced.to_string(), "5", at(20)).await;
    let repository =
        PgTokenMetadataRepository::new(setup_roles_and_connect_as(&pool, "yog_api").await);

    let page = repository.list_entries(None, 50).await.unwrap();

    assert_eq!(page.items[0].metadata.mint, unpriced);
    assert!(page.items[0].price.is_none());
    let price = page.items[1].price.as_ref().unwrap();
    assert_eq!(price.price_usd, Decimal::from(5));
    assert_eq!(price.fetched_at, at(20));
}
