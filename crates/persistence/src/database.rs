use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;
use std::time::Duration;

use crate::error::MigrationError;

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
    ///
    /// Returns `sqlx::Error` directly: connection failures at boot time are
    /// best surfaced with their original context (configuration, IO, TLS,
    /// authentication…) rather than wrapped behind a generic error type.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        Self::connect_with_options(url, 10, Duration::from_secs(5)).await
    }

    /// Connect with explicit pool sizing. The api may want a higher
    /// `max_connections` than the indexer, since requests are bursty
    /// while indexing is steady-state.
    pub async fn connect_with_options(
        url: &str,
        max_connections: u32,
        acquire_timeout: Duration,
    ) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(acquire_timeout)
            .connect(url)
            .await?;

        Ok(Self { pool })
    }

    /// Connect for a one-shot provisioning run (`yog-migrate`), with statement
    /// logging off.
    ///
    /// sqlx warns above 1s with the **whole statement** inlined. Every
    /// statement this binary runs is a file — the baseline migration alone is
    /// ~2 300 lines — so the first run against an empty database emits it
    /// verbatim as a `WARN`, roughly 100 kB of log for an event that is both
    /// expected and unactionable. Runtime services keep the warning, where a
    /// slow statement means something.
    ///
    /// One connection, not ten: it is a sequence of scripts, and the pool is
    /// dropped when the process exits.
    pub async fn connect_for_provisioning(url: &str) -> Result<Self, sqlx::Error> {
        let options = PgConnectOptions::from_str(url)?.disable_statement_logging();

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
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

    pub async fn run_migrations(&self) -> Result<(), MigrationError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(MigrationError::from)
    }

    /// Run a multi-statement provisioning script (`setup_roles.sql`,
    /// `setup_watched_pools.sql`).
    ///
    /// ⚠️ **Compiled SQL only — never a string composed at runtime.** The
    /// simple query protocol has no parameter binding, so anything
    /// interpolated into `sql` is executed as SQL. Both callers pass an
    /// `include_str!` constant, which is the only shape this method is meant
    /// to take; a value that needs interpolating belongs in a `query!` with
    /// bind parameters, not here.
    ///
    /// Uses the simple query protocol, so the whole file is sent as one
    /// statement batch — which is what lets a `DO $$ … $$` block and several
    /// `ALTER DEFAULT PRIVILEGES` travel together. It is **not** wrapped in an
    /// explicit transaction: Postgres already runs a simple-query batch as one
    /// implicit transaction, and some provisioning statements would refuse an
    /// explicit one.
    ///
    /// Unlike `run_migrations`, nothing here is versioned or recorded: these
    /// scripts are idempotent by construction and are expected to be re-run.
    pub async fn run_script(&self, sql: &str) -> Result<(), MigrationError> {
        sqlx::raw_sql(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(MigrationError::from)
    }
}
