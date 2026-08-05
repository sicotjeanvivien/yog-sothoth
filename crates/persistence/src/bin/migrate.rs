//! `yog-migrate` — one-shot binary that provisions and migrates the database.
//!
//! Lives inside the `yog-persistence` crate (as a `src/bin/` target)
//! because the migrations themselves are owned by this crate, and
//! sqlx as an engine choice must not leak elsewhere.
//!
//! # Subcommands
//!
//! ```text
//! yog-migrate                     apply pending migrations  (default)
//! yog-migrate migrate             same, spelled out
//! yog-migrate setup-roles         create the roles + structural privileges
//! yog-migrate seed-watched-pools  seed the startup allowlist
//! yog-migrate bootstrap           the three above, in order
//! yog-migrate help                this list
//! ```
//!
//! **The no-argument form must keep applying migrations**: it is what the
//! `yog-migrate` compose service and every deploy script invoke.
//!
//! # Roles, and why they differ per subcommand
//!
//! - `setup-roles` and `seed-watched-pools` read **`DATABASE_URL_ADMIN`**.
//!   Creating roles needs an admin, and the allowlist is *configuration*, not
//!   runtime data — the house convention keeps configuration writes under the
//!   admin role.
//! - `migrate` reads **`DATABASE_URL_MIGRATE`**, which must carry the
//!   `yog_migrate` role: the four runtime roles intentionally cannot alter the
//!   schema.
//!
//! `bootstrap` therefore needs both, and says so before doing anything rather
//! than failing halfway.
//!
//! # What it does not do
//!
//! It never creates the *database*. Every URL above already names one, so the
//! database must exist first (`createdb yog_sothoth`, or the compose volume).
//!
//! # Idempotency
//!
//! Every subcommand is safe to re-run: migrations skip what is applied,
//! `setup_roles.sql` guards its `CREATE ROLE`s, and the seed is
//! `ON CONFLICT DO NOTHING`.
//!
//! # Where the scripts live
//!
//! `src/bin/scripts/`, next to this file, because `include_str!` compiles them
//! *into* this binary — they ship with it and have no life of their own, unlike
//! `migrations/`, which sqlx reads as a versioned directory. Cargo ignores the
//! subdirectory (no `main.rs`, so no target is inferred from it); `cargo
//! metadata` still lists exactly one bin.
//!
//! ⚠️ Being embedded, editing a script does **not** invalidate an already-built
//! binary on its own. Same caveat as the migrations: force a rebuild (`touch`
//! this file) when a script changes, or you will run the previous version and
//! believe otherwise.

use anyhow::{Context, Result, bail};

use yog_core::domain::WatchedPoolRepository;
use yog_persistence::{Database, PgWatchedPoolRepository};

const SETUP_ROLES_SQL: &str = include_str!("scripts/setup_roles.sql");
const SETUP_WATCHED_POOLS_SQL: &str = include_str!("scripts/setup_watched_pools.sql");

const USAGE: &str = "\
yog-migrate — provision and migrate the yog-sothoth database

USAGE:
    yog-migrate [SUBCOMMAND]

SUBCOMMANDS:
    migrate                 Apply pending migrations. The default when no
                            subcommand is given.        [DATABASE_URL_MIGRATE]
    setup-roles             Create the five roles (if absent) and the
                            structural privileges.       [DATABASE_URL_ADMIN]
    seed-watched-pools      Seed the startup allowlist. Without an active row
                            a pool-centric indexer has nothing to subscribe
                            to and exits.                [DATABASE_URL_ADMIN]
    bootstrap               setup-roles, then migrate, then seed-watched-pools.
                                          [DATABASE_URL_ADMIN + _MIGRATE]
    help                    Show this message.

The database itself is never created here — it must already exist.
Every subcommand is idempotent and safe to re-run.";

#[tokio::main]
async fn main() -> Result<()> {
    // Standard env + tracing init via the shared bootstrap crate,
    // same shape as the other binaries — keeps logs uniform across
    // services and avoids re-implementing rustls/dotenv plumbing.
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    // Hand-rolled rather than clap: four subcommands, no flags, no values to
    // parse. The workspace has no argument-parsing dependency and this is not
    // enough of a reason to add one.
    let command = std::env::args().nth(1);
    match command.as_deref() {
        None | Some("migrate") => migrate().await,
        Some("setup-roles") => setup_roles().await,
        Some("seed-watched-pools") => seed_watched_pools().await,
        Some("bootstrap") => bootstrap().await,
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => {
            eprintln!("{USAGE}");
            bail!("unknown subcommand: {other}");
        }
    }
}

/// Apply pending migrations, as `yog_migrate`.
async fn migrate() -> Result<()> {
    let database = connect("DATABASE_URL_MIGRATE", "the yog_migrate role").await?;

    tracing::info!("applying pending migrations");
    database
        .run_migrations()
        .await
        .context("failed to apply migrations")?;

    tracing::info!("migrations up to date");
    Ok(())
}

/// Create the roles and the structural privileges, as the admin role.
async fn setup_roles() -> Result<()> {
    let database = connect("DATABASE_URL_ADMIN", "an admin role").await?;

    tracing::info!("applying setup_roles.sql");
    database
        .run_script(SETUP_ROLES_SQL)
        .await
        .context("failed to apply setup_roles.sql")?;

    tracing::info!("roles and structural privileges in place");
    Ok(())
}

/// Seed the startup allowlist, as the admin role.
async fn seed_watched_pools() -> Result<()> {
    let database = connect("DATABASE_URL_ADMIN", "an admin role").await?;

    tracing::info!("applying setup_watched_pools.sql");
    database
        .run_script(SETUP_WATCHED_POOLS_SQL)
        .await
        .context("failed to seed watched_pools")?;

    // The script ends with a SELECT of the active allowlist, but `run_script`
    // goes through `execute()`, which discards rows — so under the binary that
    // SELECT shows nothing, and telling the operator to "review the selection"
    // without showing it is an instruction they cannot follow. Read it back
    // through the repository rather than re-writing the SQL here.
    let repository = PgWatchedPoolRepository::new(database.pool_owned());
    let active: Vec<_> = repository
        .find_all()
        .await
        .context("failed to read back the allowlist")?
        .into_iter()
        .filter(|w| w.active)
        .collect();

    tracing::info!(count = active.len(), "watched_pools seeded");
    for watched in &active {
        tracing::info!(
            pool = %watched.pool_address,
            protocol = watched.protocol.as_str(),
            note = watched.note.as_deref().unwrap_or("—"),
            "  will be subscribed at indexer start"
        );
    }
    // INFO, not WARN. This fires on every seed, with no condition detected —
    // and the pool shipped by default is SOL-USDC, the one pick whose rationale
    // does not decay, so it would cry loudest where the risk is lowest. An
    // unconditional WARN is noise, and noise is how a real one gets ignored.
    tracing::info!(
        "review the selection before starting the indexer: a pool that has gone \
         quiet subscribes fine, logs nothing abnormal, and collects nothing"
    );
    Ok(())
}

/// The whole provisioning sequence on an existing, empty database.
async fn bootstrap() -> Result<()> {
    // Both variables are checked up front: failing on a missing
    // DATABASE_URL_MIGRATE *after* having created five cluster-wide roles
    // leaves a half-provisioned cluster and a confusing error.
    require_env("DATABASE_URL_ADMIN", "an admin role")?;
    require_env("DATABASE_URL_MIGRATE", "the yog_migrate role")?;

    tracing::info!("bootstrap 1/3 — roles and structural privileges");
    setup_roles().await?;

    tracing::info!("bootstrap 2/3 — migrations");
    migrate().await?;

    tracing::info!("bootstrap 3/3 — watched-pools allowlist");
    seed_watched_pools().await?;

    tracing::info!("bootstrap complete");
    Ok(())
}

/// Read a connection URL from the environment, naming the role it must carry.
fn require_env(var: &str, role: &str) -> Result<String> {
    std::env::var(var).with_context(|| format!("{var} must be set (credentials for {role})"))
}

/// Connect using the URL held by `var`, which must carry `role`.
async fn connect(var: &str, role: &str) -> Result<Database> {
    let url = require_env(var, role)?;

    // The URL carries a password. Log the variable it came from, not its value.
    tracing::info!("connecting with {var}");
    Database::connect_for_provisioning(&url)
        .await
        .with_context(|| format!("failed to connect using {var}"))
}
