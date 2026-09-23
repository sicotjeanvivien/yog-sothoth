//! What a dump needs to know about the server it was taken from.
//!
//! A dump restores only into the same TimescaleDB version, and `pg_dump`
//! refuses a server of a newer Postgres major than itself (see the README,
//! *Backup and restore*). `yog-archive` reads both before every dump: the
//! TimescaleDB version travels with the dump, and the Postgres major is
//! compared with its own `pg_dump`'s.
//!
//! Not a repository behind a trait in `yog-core`, for the reason
//! [`PgHealthChecker`](crate::PgHealthChecker) gives: these are facts about
//! the server, no domain aggregate is involved, and no service composes them
//! with anything else.

use sqlx::PgPool;

use yog_core::RepositoryError;

/// Reads version facts about the connected server.
#[derive(Clone)]
pub struct PgDatabaseInfo {
    pool: PgPool,
}

/// The two versions a dump depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerVersions {
    /// `server_version_num / 10000`: 16 for 16.14.
    pub postgres_major: u32,
    /// `pg_extension.extversion`, e.g. `2.27.1`.
    pub timescaledb: String,
}

impl PgDatabaseInfo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Read the Postgres major and the installed TimescaleDB version.
    ///
    /// A database without the extension is an error, not an empty version:
    /// every database of this project has it, and a dump labelled with no
    /// version could not be matched to an image that restores it.
    pub async fn server_versions(&self) -> Result<ServerVersions, RepositoryError> {
        let row = sqlx::query!(
            r#"
            SELECT current_setting('server_version_num')::int / 10000 AS "postgres_major!",
                   (SELECT extversion FROM pg_extension WHERE extname = 'timescaledb') AS timescaledb
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RepositoryError::Backend(e.to_string()))?;

        let timescaledb = row.timescaledb.ok_or_else(|| {
            RepositoryError::Integrity(
                "the timescaledb extension is not installed in this database".to_string(),
            )
        })?;
        let postgres_major = u32::try_from(row.postgres_major).map_err(|_| {
            RepositoryError::Integrity(format!(
                "server_version_num gives a negative major: {}",
                row.postgres_major
            ))
        })?;

        Ok(ServerVersions {
            postgres_major,
            timescaledb,
        })
    }
}
