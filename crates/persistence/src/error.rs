/// Failure of a migration run, or of a provisioning script.
///
/// Mirrors the sqlx error types but keeps them out of the public API
/// of `yog-persistence` — callers (like the yog-migrate binary) see a
/// thin `thiserror` enum, not the underlying engine.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    /// Anything sqlx raised while applying migrations.
    #[error("migration failed: {0}")]
    SqlxMigrate(#[from] sqlx::migrate::MigrateError),

    /// Anything sqlx raised while running a provisioning script
    /// (`setup_roles.sql`, `setup_watched_pools.sql`). Distinct from
    /// `SqlxMigrate` because these are not versioned, not tracked in
    /// `_sqlx_migrations`, and run under a different role.
    #[error("script failed: {0}")]
    Script(#[from] sqlx::Error),
}
