//! `token_last_price`: written by the statement that inserts into
//! `token_prices`, filled by migrations 017 and 018.

use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use sqlx::PgPool;
use yog_core::domain::{TokenPrice, TokenPriceRepository};
use yog_persistence::PgTokenPriceRepository;

use super::helpers::{
    CHECK_VIOLATION, pk, record_prices, setup_roles_and_connect_as, sqlstate, token_price,
};

type LastPrice = (String, Decimal, DateTime<Utc>);

fn price(mint: Pubkey, usd: i64, at: DateTime<Utc>) -> TokenPrice {
    token_price(&mint.to_string(), &usd.to_string(), at)
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
    let repository =
        PgTokenPriceRepository::new(setup_roles_and_connect_as(&pool, "yog_context").await);
    let now = Utc::now();

    for at in [now - Duration::minutes(1), now] {
        repository
            .insert_batch(&[price(pk(2), 2, at)])
            .await
            .expect("yog_context writes both tables");
    }

    assert_eq!(last_prices(&pool).await, latest_of_the_series(&pool).await);
}

/// Against 017 without its fill or past its 15-minute window, 018 without its
/// fill, keeping a row the series disagrees with, or either stamping a row with
/// another time than its price's.
#[sqlx::test(migrations = false)]
async fn the_migrations_fill_the_last_price_of_every_token(pool: PgPool) {
    let migrator = sqlx::migrate!("./migrations");
    migrator.run_to(16, &pool).await.unwrap();

    // Raw SQL: the writer of this schema does not know the table yet.
    for (mint, usd, minutes_ago) in [
        (pk(2), 1, 10),
        (pk(2), 2, 5),
        (pk(3), 9, 30),
        (pk(3), 3, 20),
        (pk(4), 4, 1),
    ] {
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

    migrator.run_to(17, &pool).await.unwrap();
    let mut window = latest_of_the_series(&pool).await;
    window.retain(|(mint, ..)| *mint != pk(3).to_string());
    assert_eq!(window.len(), 2);
    assert_eq!(last_prices(&pool).await, window);

    // One row older than the series, one newer: 018 sets both to the series.
    for (mint, minutes_ago) in [(pk(2), 8), (pk(4), -1)] {
        sqlx::query(
            "UPDATE token_last_price
                SET price_usd = 7, fetched_at = now() - $2 * INTERVAL '1 minute'
              WHERE mint = $1",
        )
        .bind(mint.to_string())
        .bind(f64::from(minutes_ago))
        .execute(&pool)
        .await
        .unwrap();
    }

    migrator.run_to(18, &pool).await.unwrap();
    let every_token = latest_of_the_series(&pool).await;
    assert_eq!(every_token.len(), 3);
    assert_eq!(last_prices(&pool).await, every_token);
}
