use sqlx::ConnectOptions;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;
use std::time::Duration;

use crate::error::MigrationError;

/// The single entry point for connecting to Postgres.
pub struct Database {
    pool: PgPool,
}

/// How [`Database::connect_with`] opens its pool. Start from
/// [`PoolSettings::DEFAULT`] and override what differs.
#[derive(Debug, Clone, Copy)]
pub struct PoolSettings {
    /// The pool size. See [`Database::DEFAULT_MAX_CONNECTIONS`].
    pub max_connections: u32,
    /// How long a caller waits for a free connection before
    /// `PoolTimedOut`. See [`Database::DEFAULT_ACQUIRE_TIMEOUT`].
    pub acquire_timeout: Duration,
    /// How long Postgres lets one statement run before cancelling it with
    /// SQLSTATE `57014`; `None` leaves the server's setting. ⚠️ Dropping a
    /// query's future stops the client waiting, not the server executing.
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
    /// How many connections [`Database::connect`] opens. ⚠️ Only the default: a
    /// caller sizing itself against the pool asks [`Database::max_connections`].
    pub const DEFAULT_MAX_CONNECTIONS: u32 = 10;

    /// How long [`Database::connect`] waits for a free connection before
    /// giving up: the deadline that turns pool contention into an error.
    pub const DEFAULT_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Connects with [`PoolSettings::DEFAULT`]. Returns [`sqlx::Error`] unwrapped,
    /// so a boot failure keeps its context (configuration, IO, TLS, authentication).
    pub async fn connect(url: &str) -> Result<Self, sqlx::Error> {
        Self::connect_with(url, PoolSettings::DEFAULT).await
    }

    /// Connect with explicit [`PoolSettings`].
    pub async fn connect_with(url: &str, settings: PoolSettings) -> Result<Self, sqlx::Error> {
        let mut options = PgConnectOptions::from_str(url)?;
        if let Some(limit) = settings.statement_timeout {
            // A startup parameter: it holds for every statement of every connection.
            options = options.options([("statement_timeout", format!("{}ms", limit.as_millis()))]);
        }

        let pool = PgPoolOptions::new()
            .max_connections(settings.max_connections)
            .acquire_timeout(settings.acquire_timeout)
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// Connects for a one-shot provisioning run (`yog-migrate`): one connection,
    /// statement logging off. ⚠️ sqlx warns above 1 s with the whole statement
    /// inlined, and every statement here is a whole migration file.
    pub async fn connect_for_provisioning(url: &str) -> Result<Self, sqlx::Error> {
        // ⚠️ A startup parameter wins over `lock_timeout` on `yog_migrate`, meant for its jobs.
        let options = PgConnectOptions::from_str(url)?
            .disable_statement_logging()
            .options([("lock_timeout", "0")]);

        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    /// How many connections this pool was actually opened with — not
    /// necessarily [`Database::DEFAULT_MAX_CONNECTIONS`].
    pub fn max_connections(&self) -> u32 {
        self.pool.options().get_max_connections()
    }

    /// Borrows the pool; cloning a `PgPool` is cheap.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn pool_owned(&self) -> PgPool {
        self.pool.clone()
    }

    /// Closes every connection and waits for them to be returned.
    pub async fn close(self) {
        self.pool.close().await;
    }

    pub async fn run_migrations(&self) -> Result<(), MigrationError> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(MigrationError::from)
    }

    /// Runs an idempotent provisioning script as one simple-query batch, unrecorded.
    /// ⚠️ Compiled SQL only: the simple protocol binds nothing, so anything interpolated
    /// is executed. Do not wrap `sql` in `AssertSqlSafe` to accept a runtime string.
    pub async fn run_script(&self, sql: &'static str) -> Result<(), MigrationError> {
        sqlx::raw_sql(sql)
            .execute(&self.pool)
            .await
            .map(|_| ())
            .map_err(MigrationError::from)
    }
}
