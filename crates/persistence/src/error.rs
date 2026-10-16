/// Failure of a migration run, or of a provisioning script.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// Anything sqlx raised while applying migrations.
    #[error("migration failed: {0}")]
    SqlxMigrate(#[from] sqlx::migrate::MigrateError),

    /// Anything sqlx raised while running a provisioning script: unversioned,
    /// and run under another role than the migrations.
    #[error("script failed: {0}")]
    Script(#[from] sqlx::Error),
}

/// Failure of a backup step: `pg_dump`, or the `pg_restore` check. Each step documents
/// which variants it returns. No message quotes the connection string.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// The connection string does not parse. The value is not quoted: it
    /// carries the password.
    #[error("the database URL is not a valid URL")]
    InvalidUrl,

    #[error("cannot run `{program}`: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },

    #[error("cannot read the major version from `{program} --version`: {output}")]
    Version { program: String, output: String },

    /// `pg_dump` refuses a server of a newer major than itself, and a dump
    /// taken by an older one is not guaranteed to restore.
    #[error(
        "pg_dump is PostgreSQL {client} and the server is PostgreSQL {server}: \
         the dump must be taken by the server's major"
    )]
    MajorMismatch { client: u32, server: u32 },

    #[error("cannot read pg_dump's output: {0}")]
    Read(std::io::Error),

    #[error("cannot wait for pg_dump: {0}")]
    DumpWait(std::io::Error),

    /// `pg_dump` ended in failure; `stderr` is the end of what it said.
    #[error("pg_dump exited with {status}: {stderr}")]
    DumpFailed {
        status: std::process::ExitStatus,
        stderr: String,
    },

    #[error("`pg_restore --file=/dev/null` did not finish: {0}")]
    CheckWait(std::io::Error),

    /// `pg_restore` cannot read the archive; `stderr` says why.
    #[error("`pg_restore --file=/dev/null` exited with {status}: {stderr}")]
    Unreadable {
        status: std::process::ExitStatus,
        stderr: String,
    },
}
