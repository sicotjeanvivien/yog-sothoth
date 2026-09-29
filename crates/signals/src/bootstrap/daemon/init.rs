//! The wiring of the materialisation alarm: every dependency
//! [`super::Daemon::new`] builds for it before it owns one.

use std::sync::Arc;

use anyhow::Context;
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};
use yog_core::domain::MaterializationBacklogRepository;

use crate::bootstrap::config::MaterializationAlarmConfig;
use crate::materialization_alarm::{
    HEARTBEAT_FAILURES, MaterializationAlarm, MaterializationAlarmSettings,
};

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
pub(super) fn init_materialization_alarm(
    repository: Arc<dyn MaterializationBacklogRepository>,
    config: &MaterializationAlarmConfig,
) -> anyhow::Result<MaterializationAlarm> {
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
