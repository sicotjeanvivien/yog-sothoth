use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::time::Duration;

pub(crate) struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new Database instance with a connection pool.
    pub(crate) async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(2)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    /// Run all pending SQL migrations.
    pub(crate) async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        Ok(())
    }

    /// Return a clone of the inner pool for use in repositories.
    pub(crate) fn pool(&self) -> PgPool {
        self.pool.clone()
    }
}
