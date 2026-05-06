use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

use crate::error::PersistenceError;

/// Thin wrapper around `sqlx::PgPool` providing a single entry point for
/// connecting and a hook for future cross-cutting concerns (metrics, health,
/// migrations runner if we ever bundle one).
///
/// The pool itself is `Clone` and cheap to clone — `Database::pool()` returns
/// a reference, but consumers needing ownership can `.clone()` the pool to
/// hand it to repositories.
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Connect to Postgres using the provided URL.
    ///
    /// Pool sizing defaults are chosen for a small-to-medium workload:
    ///   - `max_connections = 10`: enough for the indexer's concurrent task
    ///     processing or the api's request fan-out at v0.1 traffic levels.
    ///   - `acquire_timeout = 5s`: fail fast rather than queue indefinitely.
    ///
    /// Callers needing different sizing should use `connect_with_options`.
    pub async fn connect(url: &str) -> Result<Self, PersistenceError> {
        Self::connect_with_options(url, 10, Duration::from_secs(5)).await
    }

    /// Connect with explicit pool sizing. The api may want a higher
    /// `max_connections` than the indexer, since requests are bursty
    /// while indexing is steady-state.
    pub async fn connect_with_options(
        url: &str,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self, PersistenceError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
            .connect(url)
            .await?;

        Ok(Self { pool })
    }

    /// Borrow the underlying pool. Repositories that need to own a pool
    /// (the common case) should call `db.pool().clone()` — `PgPool` is an
    /// `Arc` internally, so cloning is cheap.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Convenience accessor for code that wants the pool by value.
    pub fn pool_owned(&self) -> PgPool {
        self.pool.clone()
    }
}
