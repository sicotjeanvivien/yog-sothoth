//! The wiring: every dependency [`super::Daemon::new`] builds before it owns
//! one.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};
use yog_persistence::{Database, PoolSettings};

use crate::application::metrics;

/// A statement stuck on a lock is ended by the server, and the cycle fails
/// with a reason instead of holding the next ones in silence.
const STATEMENT_TIMEOUT: Duration = Duration::from_secs(120);

/// One connection: a cycle runs its reads and its transaction in turn.
pub(super) async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    Database::connect_with(
        database_url.expose(),
        PoolSettings {
            max_connections: 1,
            statement_timeout: Some(STATEMENT_TIMEOUT),
            ..PoolSettings::DEFAULT
        },
    )
    .await
    .context("failed to connect to database")
}

pub(super) fn init_heartbeat(url: SecretUrl) -> anyhow::Result<Arc<dyn Heartbeat>> {
    let settings = HeartbeatSettings {
        variable: "ANALYTIC_HEARTBEAT_URL",
        undelivered_counter: metrics::HEARTBEAT_FAILURES,
    };
    let heartbeat = HealthchecksHeartbeat::new(url, settings)
        .context("failed to build the heartbeat client")?;
    Ok(Arc::new(heartbeat))
}
