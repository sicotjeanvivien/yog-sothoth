//! Database connectivity probe for the api's readiness handler.

use std::time::Duration;

use sqlx::PgPool;
use tokio::time::timeout;

/// How long a ping waits before the database counts as unreachable: a
/// readiness check that hangs blocks the load balancer's decision.
const PING_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct PgHealthChecker {
    pool: PgPool,
}

impl PgHealthChecker {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// A `SELECT 1` round-trip within `PING_TIMEOUT`.
    pub async fn ping(&self) -> Result<(), HealthError> {
        let fut = sqlx::query("SELECT 1").execute(&self.pool);

        match timeout(PING_TIMEOUT, fut).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(HealthError::Query(e.to_string())),
            Err(_) => Err(HealthError::Timeout),
        }
    }
}

/// Why a health check failed. The string variant swallows the [`sqlx::Error`],
/// so no internals reach the readiness payload.
#[derive(Debug)]
pub enum HealthError {
    Timeout,
    Query(String),
}
