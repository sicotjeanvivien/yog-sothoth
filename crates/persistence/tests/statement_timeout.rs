//! `PoolSettings::statement_timeout`: Postgres itself cancels a statement that outruns it,
//! with SQLSTATE `57014` — and the same statement passes without the setting.

use std::time::Duration;

use sqlx::{ConnectOptions, PgPool};
use yog_persistence::{Database, PoolSettings};

use super::helpers::sqlstate;

/// `57014` — query_canceled.
const QUERY_CANCELED: &str = "57014";

/// Longer than the limit below, by a margin that no scheduling jitter covers.
const SLEEP: &str = "SELECT pg_sleep(0.5)";

/// A second pool on the test's own database, with `settings`.
async fn reconnect(pool: &PgPool, settings: PoolSettings) -> Database {
    let url = pool.connect_options().to_url_lossy();
    Database::connect_with(url.as_str(), settings)
        .await
        .expect("connect to the test database")
}

#[sqlx::test]
async fn a_statement_past_the_limit_is_cancelled_by_postgres(pool: PgPool) {
    let db = reconnect(
        &pool,
        PoolSettings {
            statement_timeout: Some(Duration::from_millis(100)),
            ..PoolSettings::DEFAULT
        },
    )
    .await;

    let err = sqlx::query(SLEEP)
        .execute(db.pool())
        .await
        .expect_err("the sleep outlasts statement_timeout");

    assert_eq!(sqlstate(&err), QUERY_CANCELED);
}

#[sqlx::test]
async fn without_the_limit_the_same_statement_completes(pool: PgPool) {
    let db = reconnect(&pool, PoolSettings::DEFAULT).await;

    sqlx::query(SLEEP)
        .execute(db.pool())
        .await
        .expect("no statement_timeout, no cancellation");
}
