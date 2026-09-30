//! The wiring of the materialisation alarm: every dependency
//! [`super::Daemon::new`] builds for it before it owns one.

use std::sync::Arc;

use anyhow::Context;
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};
use yog_core::domain::MaterializationBacklogRepository;
use yog_persistence::{Database, PgMaterializationBacklogRepository, PoolSettings};

use crate::application::workers::{
    HEARTBEAT_FAILURES, MaterializationAlarm, MaterializationAlarmSettings, STATEMENT_TIMEOUT,
};
use crate::bootstrap::config::MaterializationAlarmConfig;

/// The Healthchecks.io check, when one is configured. A URL that cannot take
/// `/fail` stops the daemon here, with the variable named.
fn init_heartbeat(url: SecretUrl) -> anyhow::Result<Arc<dyn Heartbeat>> {
    let settings = HeartbeatSettings {
        variable: "SIGNALS_MATERIALIZATION_HEARTBEAT_URL",
        undelivered_counter: HEARTBEAT_FAILURES,
    };
    let heartbeat = HealthchecksHeartbeat::new(url, settings)
        .context("failed to build the heartbeat client")?;
    Ok(Arc::new(heartbeat))
}

/// The alarm over the backlogs, reporting to its check if there is one.
///
/// It reads through **its own pool**, of one connection, whose
/// `statement_timeout` lets Postgres cancel a read stuck on a lock. Sharing the
/// detectors' pool, a stuck read would hold a connection they need — and one
/// more at every check, since dropping the read on the client side leaves the
/// statement running on the server.
pub(super) async fn init_materialization_alarm(
    database_url: &SecretUrl,
    config: &MaterializationAlarmConfig,
) -> anyhow::Result<MaterializationAlarm> {
    let database = Database::connect_with(
        database_url.expose(),
        PoolSettings {
            max_connections: 1,
            statement_timeout: Some(STATEMENT_TIMEOUT),
            ..PoolSettings::DEFAULT
        },
    )
    .await
    .context("failed to connect the materialisation alarm to the database")?;
    let repository: Arc<dyn MaterializationBacklogRepository> = Arc::new(
        PgMaterializationBacklogRepository::new(database.pool().clone()),
    );

    let heartbeat = config
        .heartbeat_url
        .clone()
        .map(init_heartbeat)
        .transpose()?;
    Ok(MaterializationAlarm::new(
        repository,
        heartbeat,
        MaterializationAlarmSettings {
            interval: config.interval,
            max_wait: config.max_wait,
        },
    ))
}
