//! `PoolSettings::statement_timeout`: Postgres itself cancels a statement that
//! outruns it, with SQLSTATE `57014`.
//!
//! The limit is the one bound a caller cannot enforce from the client side —
//! dropping a query's future stops the wait, not the server. Proven here by
//! the statement that fails, and by the same statement passing without the
//! setting, so the failure is pinned on the setting and on nothing else.

use std::time::Duration;

use sqlx::{ConnectOptions, PgPool};
use yog_persistence::{Database, PoolSettings};

use super::helpers::sqlstate;

/// `57014` — query_canceled, what a statement stopped by `statement_timeout`
/// fails with.
const QUERY_CANCELED: &str = "57014";

/// Longer than the limit below, by a margin that no scheduling jitter covers.
const SLEEP: &str = "SELECT pg_sleep(0.5)";

/// Open a second pool on the test's own database, with `settings`.
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
