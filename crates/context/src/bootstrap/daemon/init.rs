//! The wiring: every dependency [`super::Daemon::connect`] builds before it owns
//! one, and the metric families it declares.

use std::sync::Arc;

use anyhow::Context;
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};
use yog_persistence::Database;

use super::log;
use crate::application::workers::{MetadataWorkerMetrics, PriceWorkerMetrics};
use crate::infra::{HEARTBEAT_FAILURES, ProviderMetrics};

/// Connect to the database. The error names the failure, never the URL.
pub(super) async fn init_db(database_url: &SecretUrl) -> anyhow::Result<Database> {
    let db = Database::connect(database_url.expose())
        .await
        .context("failed to connect to database")?;
    log::connected_to_database();
    Ok(db)
}

pub(super) fn init_heartbeat(url: SecretUrl) -> anyhow::Result<Arc<dyn Heartbeat>> {
    let settings = HeartbeatSettings {
        variable: "CONTEXT_HEARTBEAT_URL",
        undelivered_counter: HEARTBEAT_FAILURES,
    };
    let heartbeat = HealthchecksHeartbeat::new(url, settings)
        .context("failed to build the heartbeat client")?;
    Ok(Arc::new(heartbeat))
}

/// Declare the HELP text of every metric family the process exports.
pub(super) fn register_metric_descriptions() {
    MetadataWorkerMetrics::register_descriptions();
    PriceWorkerMetrics::register_descriptions();
    ProviderMetrics::register_descriptions();
}
