use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;
use std::time::Duration;

use crate::error::MigrationError;

/// Thin wrapper around [`sqlx::PgPool`] providing a single entry point for
/// connecting and a hook for future cross-cutting concerns (metrics, health,
/// migrations runner if we ever bundle one).
///
/// The pool itself is `Clone` and cheap to clone — `Database::pool()` returns
/// a reference, but consumers needing ownership can `.clone()` the pool to
/// hand it to repositories.
pub struct Database {
    pool: PgPool,
}

/// How [`Database::connect_with`] opens its pool.
///
/// Start from [`PoolSettings::DEFAULT`] and override what differs, so that a
/// caller names only the settings it has a reason for.
#[derive(Debug, Clone, Copy)]
pub struct PoolSettings {
    /// The pool size. See [`Database::DEFAULT_MAX_CONNECTIONS`].
    pub max_connections: u32,
    /// How long a caller waits for a free connection before
    /// `PoolTimedOut`. See [`Database::DEFAULT_ACQUIRE_TIMEOUT`].
    pub acquire_timeout: Duration,
    /// How long Postgres lets one statement run before cancelling it with
    /// SQLSTATE `57014`. `None` leaves the server's setting (no limit, by
    /// default).
    ///
    /// The one bound a caller cannot build on its own: dropping a query's
    /// future stops the client waiting, not the server executing. A runaway
    /// statement keeps its connection and its CPU until Postgres ends it.
    pub statement_timeout: Option<Duration>,
}

impl PoolSettings {
    /// What [`Database::connect`] uses.
    pub const DEFAULT: Self = Self {
        max_connections: Database::DEFAULT_MAX_CONNECTIONS,
        acquire_timeout: Database::DEFAULT_ACQUIRE_TIMEOUT,
        statement_timeout: None,
    };
}

impl Database {
    /// How many connections [`Database::connect`] opens.
    ///
    /// It is only the **default**: a caller sizing itself against the pool must
    /// ask [`Database::max_connections`], which is the pool that was actually
    /// opened. This constant says what `connect` uses when nobody chose;
    /// reading it as "the pool size" is wrong the moment someone calls
    /// [`Database::connect_with`].
    ///
    /// Public so that the two can be compared and so that a caller can size a
    /// pool deliberately — not as a number for other components to copy.
    pub const DEFAULT_MAX_CONNECTIONS: u32 = 10;

    /// How long [`Database::connect`] waits for a free connection before
    /// giving up — what a caller whose statements out-number the pool hits.
    ///
    /// Named rather than inlined because it is the deadline that turns pool
    /// contention into an error, and a component reasoning about that
    /// contention should be able to name the number it is racing.
    pub const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Connect to Postgres using the provided URL.
    ///
    /// Pool sizing defaults are chosen for a small-to-medium workload:
    ///   - [`Database::DEFAULT_MAX_CONNECTIONS`]: enough for the indexer's
    ///     concurrent task processing or the api's request fan-out at v0.1
    ///     traffic levels.
    ///   - [`Database::DEFAULT_ACQUIRE_TIMEOUT`]: fail fast rather than queue
    ///     indefinitely.
    ///
    /// Callers needing different settings use [`Database::connect_with`].
    ///
    /// Returns [`sqlx::Error`] directly: connection failures at boot time are
    /// best surfaced with their original context (configuration, IO, TLS,
    /// authentication…) rather than wrapped behind a generic error type.
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        Self::connect_with(url, PoolSettings::DEFAULT).await
    }

    /// Connect with explicit [`PoolSettings`].
    pub async fn connect_with(url: &str, settings: PoolSettings) -> Result<Self, sqlx::Error> {
        let mut options = PgConnectOptions::from_str(url)?;
        if let Some(limit) = settings.statement_timeout {
            // Sent as a startup parameter, so it holds for every statement of
            // every connection the pool opens. sqlx escapes the value itself
            // since 0.9.
            options = options.options([("statement_timeout", format!("{}ms", limit.as_millis()))]);
        }

        let pool = PgPoolOptions::new()
            .max_connections(settings.max_connections)
            .acquire_timeout(settings.acquire_timeout)
            .connect_with(options)
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

    /// How many connections this pool was actually opened with.
    ///
    /// ⚠️ **Read this, do not assume [`Database::DEFAULT_MAX_CONNECTIONS`].**
    /// A caller sizing itself against the pool — the indexer's bounded worker
    /// does — must ask the pool it was handed, or the two silently part ways
    /// the day someone calls [`Database::connect_with`]. The constant is the
    /// default; this is the fact.
    pub fn max_connections(&self) -> u32 {
        self.pool.options().get_max_connections()
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

    /// Close every connection and wait for them to be returned. For a caller
    /// that connects for one piece of work and must not hold a connection
    /// afterwards.
    pub async fn close(self) {
        self.pool.close().await;
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
    /// to take — and since sqlx 0.9 the `&'static str` says so: `raw_sql`
    /// refuses a borrowed string unless it is wrapped in `AssertSqlSafe`, and
    /// wrapping it here would sign that promise for every future caller. A
    /// value that needs interpolating belongs in a `query!` with bind
    /// parameters, not here.
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
    pub async fn run_script(&self, sql: &'static str) -> Result<(), MigrationError> {
        sqlx::raw_sql(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(MigrationError::from)
    }
}
