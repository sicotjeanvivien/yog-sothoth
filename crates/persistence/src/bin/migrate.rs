//! `yog-migrate`: provisions and migrates the database (subcommands in `USAGE`).
//! `setup-roles` and `seed-watched-pools` connect with `DATABASE_URL_ADMIN`, `migrate` and the
//! API-key commands with `DATABASE_URL_MIGRATE`. ⚠️ The no-argument form must keep migrating: compose and every
//! deploy script invoke it. It never creates the database.

use anyhow::{Context, Result, anyhow, bail};

use yog_bootstrap::SecretUrl;
use yog_core::domain::{
    API_KEY_PREFIX_BYTES, API_KEY_SECRET_BYTES, ApiKey, WatchedPoolRepository, is_client_name,
};
use yog_persistence::{Database, PgApiKeyRepository, PgWatchedPoolRepository};

const SETUP_ROLES_SQL: &str = include_str!("scripts/setup_roles.sql");
const SETUP_WATCHED_POOLS_SQL: &str = include_str!("scripts/setup_watched_pools.sql");

const USAGE: &str = "\
yog-migrate — provision and migrate the yog-sothoth database

USAGE:
    yog-migrate [SUBCOMMAND]

SUBCOMMANDS:
    migrate                 Apply pending migrations. The default when no
                            subcommand is given.        [DATABASE_URL_MIGRATE]
    setup-roles             Create the six roles (if absent) and the
                            structural privileges.       [DATABASE_URL_ADMIN]
    seed-watched-pools      Seed the startup allowlist. Without an active row
                            a pool-centric indexer has nothing to subscribe
                            to and exits.                [DATABASE_URL_ADMIN]
    bootstrap               setup-roles, then migrate, then seed-watched-pools.
                                          [DATABASE_URL_ADMIN + _MIGRATE]
    create-api-key <client> Issue a new key to <client>, created on its first
                            key. The key is printed once, on stdout, and
                            never stored: only its hash is.
                                                       [DATABASE_URL_MIGRATE]
    revoke-api-key <prefix> Revoke the key named by its prefix.
                                                       [DATABASE_URL_MIGRATE]
    list-api-keys           Every key: prefix, client, created, revoked.
                                                       [DATABASE_URL_MIGRATE]
    help                    Show this message.

The database itself is never created here — it must already exist.
Every subcommand but create-api-key is idempotent and safe to re-run;
create-api-key issues a new key on each run.";

#[tokio::main]
async fn main() -> Result<()> {
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    // ⚠️ On stderr: `create-api-key` prints its key alone on stdout, so `KEY=$(…)` captures it whole.
    yog_bootstrap::init_tracing_on_stderr();

    // Hand-rolled: a few subcommands and no flags do not justify an argument-parsing dependency.
    let command = std::env::args().nth(1);
    match command.as_deref() {
        None | Some("migrate") => migrate().await,
        Some("setup-roles") => setup_roles().await,
        Some("seed-watched-pools") => seed_watched_pools().await,
        Some("bootstrap") => bootstrap().await,
        Some("create-api-key") => create_api_key(&argument("<client>")?).await,
        Some("revoke-api-key") => revoke_api_key(&argument("<prefix>")?).await,
        Some("list-api-keys") => list_api_keys().await,
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

    // ⚠️ `run_script` discards the rows of the script's closing SELECT: the
    // allowlist is read back so the operator sees what to review.
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
    // INFO, not WARN: it fires on every seed, with nothing detected.
    tracing::info!(
        "review the selection before starting the indexer: a pool that has gone \
         quiet subscribes fine, logs nothing abnormal, and collects nothing"
    );
    Ok(())
}

/// The whole provisioning sequence on an existing, empty database.
async fn bootstrap() -> Result<()> {
    // Both checked up front: failing after the cluster-wide roles exist leaves
    // a half-provisioned cluster.
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

/// Issue a new key to `client`, as `yog_migrate`.
async fn create_api_key(client: &str) -> Result<()> {
    if !is_client_name(client) {
        bail!("a client is named by 1 to 63 of [a-z0-9-], a letter or a digit first: {client:?}");
    }
    let database = connect("DATABASE_URL_MIGRATE", "the yog_migrate role").await?;

    let mut prefix = [0u8; API_KEY_PREFIX_BYTES];
    let mut secret = [0u8; API_KEY_SECRET_BYTES];
    getrandom::fill(&mut prefix)
        .and_then(|()| getrandom::fill(&mut secret))
        .map_err(|e| anyhow!("the OS random source failed: {e}"))?;
    let key = ApiKey::from_random(prefix, secret);

    PgApiKeyRepository::new(database.pool_owned())
        .create(client, &key)
        .await
        .context("failed to store the key")?;

    tracing::info!(client, prefix = key.prefix(), "api key issued");
    // ⚠️ Never through `tracing`: the key must not reach a log.
    println!("{}", key.expose());
    Ok(())
}

/// Revoke the key named by `prefix`, as `yog_migrate`.
async fn revoke_api_key(prefix: &str) -> Result<()> {
    let database = connect("DATABASE_URL_MIGRATE", "the yog_migrate role").await?;

    let revoked = PgApiKeyRepository::new(database.pool_owned())
        .revoke(prefix)
        .await
        .context("failed to revoke the key")?;
    if !revoked {
        bail!("no active key under prefix {prefix}");
    }
    tracing::info!(prefix, "api key revoked");
    Ok(())
}

/// Every key, as `yog_migrate`: never a key, never a hash.
async fn list_api_keys() -> Result<()> {
    let database = connect("DATABASE_URL_MIGRATE", "the yog_migrate role").await?;

    let keys = PgApiKeyRepository::new(database.pool_owned())
        .list()
        .await
        .context("failed to list the keys")?;
    for key in &keys {
        let revoked = key
            .revoked_at
            .map_or_else(|| "active".to_string(), |at| format!("revoked {at}"));
        println!(
            "{} {} created {} {revoked}",
            key.prefix, key.client, key.created_at
        );
    }
    Ok(())
}

/// The subcommand's argument, which it cannot do without.
fn argument(name: &str) -> Result<String> {
    match std::env::args().nth(2) {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => {
            eprintln!("{USAGE}");
            bail!("missing argument {name}")
        }
    }
}

/// Reads a connection URL, naming the role it must carry. Through
/// [`yog_bootstrap::required_secret_url`], not [`std::env::var`]: a blank value is
/// missing, and the trailing `\r` of a CRLF `.env` is trimmed.
fn require_env(var: &str, role: &str) -> Result<SecretUrl> {
    yog_bootstrap::required_secret_url(var)
        .with_context(|| format!("{var} must be set (credentials for {role})"))
}

/// Connects with the URL held by `var`, which must carry `role`. The URL stays a
/// [`SecretUrl`] until the connection call, so an error chain prints it redacted.
async fn connect(var: &str, role: &str) -> Result<Database> {
    let url = require_env(var, role)?;

    tracing::info!("connecting with {var}");
    Database::connect_for_provisioning(url.expose())
        .await
        .with_context(|| format!("failed to connect using {var}"))
}

/// The rules `yog-migrate` imposes on the files it applies — chiefly that every
/// index is named, so Postgres never has to invent a name it will truncate.
/// Test-only: nothing here is compiled into the binary that runs.
#[cfg(test)]
#[path = "migrate/lint.rs"]
mod lint;
