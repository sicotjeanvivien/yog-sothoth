//! `yog-migrate` — one-shot binary that applies pending database
//! migrations.
//!
//! Lives inside the `yog-persistence` crate (as a `src/bin/` target)
//! because the migrations themselves are owned by this crate, and
//! sqlx as an engine choice must not leak elsewhere.
//!
//! # Runtime contract
//!
//! - Reads a single env var `DATABASE_URL`. It must point at a
//!   connection string for the `yog_migrate` role (DDL privileges),
//!   NOT for any of the runtime roles which intentionally cannot
//!   alter the schema.
//! - Connects, applies pending migrations, exits 0.
//! - On any failure (connection, DDL, role-permission), exits 1
//!   after logging the cause.
//!
//! Idempotent: running it twice in a row is a no-op the second time.
//! Safe to wire as the very first step of `docker compose up`, of a
//! CI/CD deploy script, or of a manual local workflow.

use anyhow::{Context, Result};

use yog_persistence::Database;

#[tokio::main]
async fn main() -> Result<()> {
    // Standard env + tracing init via the shared bootstrap crate,
    // same shape as the other binaries — keeps logs uniform across
    // services and avoids re-implementing rustls/dotenv plumbing.
    yog_bootstrap::init_rustls();
    dotenvy::dotenv().ok();
    yog_bootstrap::init_tracing();

    let database_url = std::env::var("DATABASE_URL_MIGRATE")
        .context("DATABASE_URL must be set (with credentials for the yog_migrate role)")?;

    tracing::info!("connecting to database for migration : {}", database_url);
    let database = Database::connect(&database_url)
        .await
        .context("failed to connect to database")?;

    tracing::info!("applying pending migrations");
    database
        .run_migrations()
        .await
        .context("failed to apply migrations")?;

    tracing::info!("migrations up to date");
    Ok(())
}
