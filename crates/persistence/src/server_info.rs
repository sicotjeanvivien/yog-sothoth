//! Facts about the server itself, as opposed to the data it holds.
//!
//! Not a repository behind a trait in `yog-core`, for the reason
//! [`PgHealthChecker`](crate::PgHealthChecker) gives: no domain aggregate is
//! involved, and no service composes these facts with anything else. Kept out
//! of `database.rs`, which opens and closes connections and runs no query of
//! its own.

use sqlx::PgPool;

use yog_core::RepositoryError;

/// The two versions a dump depends on.
///
/// A dump restores only into the same TimescaleDB version, and `pg_dump`
/// refuses a server of a newer Postgres major than itself (see the README,
/// *Backup and restore*). `yog-archive` reads both before every dump: the
/// TimescaleDB version travels with the dump, and the Postgres major is
/// compared with `pg_dump`'s by [`PgTools::ensure_matches`](crate::PgTools::ensure_matches).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerVersions {
    /// `server_version_num / 10000`: 16 for 16.14.
    pub postgres_major: u32,
    /// `pg_extension.extversion`, e.g. `2.27.1`.
    pub timescaledb: String,
}

/// Reads facts about the server a pool reaches. It connects nothing itself:
/// the caller hands it a pool, and decides how long that pool lives.
#[derive(Clone)]
pub struct PgServerInfo {
    pool: PgPool,
}

impl PgServerInfo {
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
