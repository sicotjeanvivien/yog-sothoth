//! The wiring: every dependency [`super::Daemon::new`] builds before it owns
//! one.

use std::sync::Arc;

use anyhow::Context;
use object_store::ObjectStore;
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};

use crate::{bootstrap::config::StoreConfig, infra::open_store, metrics};

/// The heartbeat comes first: from here on, whatever fails can be told.
pub(super) fn init_heartbeat(url: SecretUrl) -> anyhow::Result<HealthchecksHeartbeat> {
    let settings = HeartbeatSettings {
        variable: "ARCHIVE_HEARTBEAT_URL",
        undelivered_counter: metrics::HEARTBEAT_FAILURES,
    };
    HealthchecksHeartbeat::new(url, settings).context("failed to build the heartbeat client")
}

/// Open the bucket, or signal why it cannot be before the process stops: a
/// daemon that exits on a bad store configuration without a word would look,
/// from Healthchecks.io, exactly like one that was never started.
pub(super) async fn init_store(
    config: &StoreConfig,
    heartbeat: &dyn Heartbeat,
) -> anyhow::Result<Arc<dyn ObjectStore>> {
    match open_store(config) {
        Ok(store) => Ok(store),
        Err(e) => {
            heartbeat
                .failure(&format!("store_misconfigured: {e}"))
                .await;
            Err(anyhow::Error::new(e).context("failed to configure the bucket"))
        }
    }
}
