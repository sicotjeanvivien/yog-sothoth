//! Daemon assembly: the database, the heartbeat, and the worker they feed.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tokio_util::sync::CancellationToken;
use yog_bootstrap::{HealthchecksHeartbeat, Heartbeat, HeartbeatSettings, SecretUrl};
use yog_core::domain::AnalyticRepository;
use yog_persistence::{Database, PgAnalyticRepository, PoolSettings};

use crate::application::{AnalyticWorker, metrics};
use crate::bootstrap::Config;

/// A statement stuck on a lock is ended by the server, and the cycle fails
/// with a reason instead of holding the next ones in silence.
const STATEMENT_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) struct Daemon {
    worker: AnalyticWorker,
}

impl Daemon {
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let heartbeat = config.heartbeat_url.map(init_heartbeat).transpose()?;
        // One connection: a cycle runs its reads and its transaction in turn.
        let database = Database::connect_with(
            config.database_url.expose(),
            PoolSettings {
                max_connections: 1,
                statement_timeout: Some(STATEMENT_TIMEOUT),
                ..PoolSettings::DEFAULT
            },
        )
        .await
        .context("failed to connect to database")?;
        let repository: Arc<dyn AnalyticRepository> =
            Arc::new(PgAnalyticRepository::new(database.pool_owned()));

        Ok(Self {
            worker: AnalyticWorker::new(repository, heartbeat, config.cycle),
        })
    }

    pub(crate) async fn run(self, shutdown: CancellationToken) {
        self.worker.run(shutdown).await;
    }
}

fn init_heartbeat(url: SecretUrl) -> anyhow::Result<Arc<dyn Heartbeat>> {
    let settings = HeartbeatSettings {
        variable: "ANALYTIC_HEARTBEAT_URL",
        undelivered_counter: metrics::HEARTBEAT_FAILURES,
    };
    let heartbeat = HealthchecksHeartbeat::new(url, settings)
        .context("failed to build the heartbeat client")?;
    Ok(Arc::new(heartbeat))
}
