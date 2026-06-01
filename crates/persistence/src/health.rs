//! Database connectivity probe.
//!
//! Not a repository in the DDD sense — it manipulates no domain
//! aggregate, just answers "is the DB reachable right now?". Lives
//! in `yog-persistence` rather than behind a trait in `yog-core`
//! because there is no application-layer abstraction to factor out:
//! `SELECT 1` succeeds or it doesn't, and no service composes its
//! result with anything else.
//!
//! Exposed publicly so `yog-api` can construct one in its bootstrap
//! and call `ping()` from the readiness handler directly.

use std::time::Duration;

use sqlx::PgPool;
use tokio::time::timeout;

/// Maximum time we wait for the DB to acknowledge a ping before
/// declaring it unreachable. Capped low because a readiness check
/// that hangs blocks the load balancer's decision loop — better to
/// flip to "not ready" fast than to wait for a slow DB.
const PING_TIMEOUT: Duration = Duration::from_secs(1);

/// Probe the database connection for liveness from the API process.
///
/// Holds a `PgPool` clone (cheap — `PgPool` is `Arc` internally) and
/// exposes a single `ping()` method. Stored on `AppState` so the
/// readiness handler can call it.
#[derive(Clone)]
pub struct PgHealthChecker {
    pool: PgPool,
}

impl PgHealthChecker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run a minimal round-trip against the database.
    ///
    /// Returns `Ok(())` when Postgres acknowledges within
    /// [`PING_TIMEOUT`], `Err(HealthError)` otherwise. The error
    /// variant distinguishes a timeout (DB slow or unreachable) from
    /// a connection-level failure (DB returned an error).
    pub async fn ping(&self) -> Result<(), HealthError> {
        let fut = sqlx::query("SELECT 1").execute(&self.pool);

        match timeout(PING_TIMEOUT, fut).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(HealthError::Query(e.to_string())),
            Err(_) => Err(HealthError::Timeout),
        }
    }
}

/// Reasons a health check can fail.
///
/// The string variant intentionally swallows the underlying
/// `sqlx::Error` to avoid leaking internals through the readiness
/// payload — the handler maps both variants to a generic "db down"
/// label.
#[derive(Debug)]
pub enum HealthError {
    Timeout,
    Query(String),
}
