//! Daemon assembly: the database, the heartbeat, and the worker they feed.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use yog_core::domain::AnalyticRepository;
use yog_persistence::PgAnalyticRepository;

use crate::application::AnalyticWorker;
use crate::bootstrap::Config;

mod init;

use init::{init_database, init_heartbeat};

pub(crate) struct Daemon {
    worker: AnalyticWorker,
}

impl Daemon {
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let heartbeat = config.heartbeat_url.map(init_heartbeat).transpose()?;
        let database = init_database(&config.database_url).await?;
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
