/// Failure of a migration run.
///
/// Mirrors `sqlx::migrate::MigrateError` but keeps the sqlx type out
/// of the public API of `yog-persistence` — callers (like the
/// yog-migrate binary) see a thin `thiserror` enum, not the
/// underlying engine.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// Anything sqlx raised while applying migrations.
    #[error("migration failed: {0}")]
    SqlxMigrate(#[from] sqlx::migrate::MigrateError),
}
