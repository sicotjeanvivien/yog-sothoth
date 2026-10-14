//! `token_last_price` (017): written by the statement that inserts into
//! `token_prices`, read by `pool_current_tvl`, filled by the migration from the
//! last 15 minutes of prices.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::domain::{PriceProvider, TokenPrice, TokenPriceRepository};
use yog_persistence::PgTokenPriceRepository;

use super::helpers::{
    CHECK_VIOLATION, INSUFFICIENT_PRIVILEGE, as_role, pk, record_prices, sqlstate,
};

type LastPrice = (String, Decimal, DateTime<Utc>);

fn price(mint: Pubkey, usd: i64, at: DateTime<Utc>) -> TokenPrice {
    TokenPrice {
        mint,
        price_usd: Decimal::new(usd, 0),
        price_provider: PriceProvider::Jupiter,
        confidence: None,
        fetched_at: at,
    }
}

async fn last_prices(pool: &PgPool) -> Vec<LastPrice> {
    sqlx::query_as("SELECT mint, price_usd, fetched_at FROM token_last_price ORDER BY mint")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn latest_of_the_series(pool: &PgPool) -> Vec<LastPrice> {
    sqlx::query_as(
        "SELECT DISTINCT ON (mint) mint, price_usd, fetched_at
           FROM token_prices ORDER BY mint, fetched_at DESC",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Against the upsert without `DISTINCT ON` (21000), or ordered the wrong way.
#[sqlx::test]
async fn a_batch_keeps_the_latest_price_of_each_token(pool: PgPool) {
    let now = Utc::now();
    record_prices(
        &pool,
        &[
            price(pk(2), 1, now - Duration::minutes(1)),
            price(pk(2), 2, now),
            price(pk(3), 7, now - Duration::minutes(2)),
        ],
    )
    .await;

    let last = last_prices(&pool).await;
    assert_eq!(last, latest_of_the_series(&pool).await);
    let a = last.iter().find(|(mint, ..)| *mint == pk(2).to_string());
    assert_eq!(a.map(|(_, usd, _)| *usd), Some(Decimal::new(2, 0)));
}

/// Against the upsert without its `WHERE`, or with the comparison reversed.
#[sqlx::test]
async fn an_older_price_never_replaces_a_newer_one(pool: PgPool) {
    let now = Utc::now();
    record_prices(&pool, &[price(pk(2), 2, now)]).await;

    record_prices(&pool, &[price(pk(2), 9, now - Duration::minutes(5))]).await;
    assert_eq!(last_prices(&pool).await[0].1, Decimal::new(2, 0));
    assert_eq!(last_prices(&pool).await, latest_of_the_series(&pool).await);

    record_prices(&pool, &[price(pk(2), 3, now + Duration::seconds(1))]).await;
    assert_eq!(last_prices(&pool).await[0].1, Decimal::new(3, 0));
}

/// Against the table without its `CHECK`.
#[sqlx::test]
async fn a_zero_price_is_refused(pool: PgPool) {
    let err = sqlx::query(
        "INSERT INTO token_last_price (mint, price_usd, fetched_at) VALUES ('m', 0, now())",
    )
    .execute(&pool)
    .await
    .expect_err("a zero price values nothing");

    assert_eq!(sqlstate(&err), CHECK_VIOLATION);
    let message = err.as_database_error().unwrap().message().to_owned();
    assert!(
        message.contains("token_last_price_price_usd_positive"),
        "{message}"
    );
}

/// Against 017 granting `yog_context` less than `SELECT, INSERT, UPDATE`: the
/// second batch updates the row the first inserted.
#[sqlx::test]
async fn the_price_worker_writes_the_last_price_as_yog_context(pool: PgPool) {
    let repository = PgTokenPriceRepository::new(as_role(&pool, "yog_context").await);
    let now = Utc::now();

    for at in [now - Duration::minutes(1), now] {
        repository
            .insert_batch(&[price(pk(2), 2, at)])
            .await
            .expect("yog_context writes both tables");
    }

    assert_eq!(last_prices(&pool).await, latest_of_the_series(&pool).await);
}

#[sqlx::test]
async fn no_other_runtime_role_writes_the_last_price(pool: PgPool) {
    for role in [
        "yog_indexer",
        "yog_api",
        "yog_signals",
        "yog_archive",
        "yog_analytic",
    ] {
        let scoped = as_role(&pool, role).await;
        let err = sqlx::query(
            "INSERT INTO token_last_price (mint, price_usd, fetched_at) VALUES ('m', 1, now())",
        )
        .execute(&scoped)
        .await
        .expect_err("only yog_context writes the last price");

        assert_eq!(sqlstate(&err), INSUFFICIENT_PRIVILEGE, "{role}");
        let message = err.as_database_error().unwrap().message().to_owned();
        assert!(
            message.contains("table token_last_price"),
            "{role}: {message}"
        );
    }
}

/// Against 017 without its fill, or filling past the 15-minute window.
#[sqlx::test(migrations = false)]
async fn the_migration_fills_the_last_price_from_the_window(pool: PgPool) {
    let migrator = sqlx::migrate!("./migrations");
    migrator.run_to(16, &pool).await.unwrap();

    // Raw SQL: the writer of this schema does not know the table yet.
    for (mint, usd, minutes_ago) in [(pk(2), 1, 10), (pk(2), 2, 5), (pk(3), 3, 20), (pk(4), 4, 1)] {
        sqlx::query(
            "INSERT INTO token_prices (mint, price_usd, price_provider, fetched_at)
             VALUES ($1, $2, 'jupiter', now() - $3 * INTERVAL '1 minute')",
        )
        .bind(mint.to_string())
        .bind(Decimal::new(usd, 0))
        .bind(f64::from(minutes_ago))
        .execute(&pool)
        .await
        .unwrap();
    }

    migrator.run(&pool).await.unwrap();

    let mut filled: Vec<(String, Decimal)> = last_prices(&pool)
        .await
        .into_iter()
        .map(|(mint, usd, _)| (mint, usd))
        .collect();
    filled.sort();
    let mut expected = vec![
        (pk(2).to_string(), Decimal::new(2, 0)),
        (pk(4).to_string(), Decimal::new(4, 0)),
    ];
    expected.sort();
    assert_eq!(filled, expected);
}
