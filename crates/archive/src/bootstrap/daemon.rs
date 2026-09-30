//! Daemon assembly: build the store, the heartbeat and the archiver, then hand
//! them to the [`ArchiveWorker`], which runs until asked to stop.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use yog_persistence::PgTools;

use crate::{
    application::{ArchiveWorker, Archiver},
    bootstrap::Config,
    infra::PgVersions,
};

mod init;

use init::{init_heartbeat, init_store};

pub(crate) struct Daemon {
    worker: ArchiveWorker,
}

impl Daemon {
    /// Build everything a run needs. Nothing here touches the database: see
    /// [`PgVersions`] for why the connection belongs to the run.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let heartbeat = init_heartbeat(config.heartbeat_url)?;
        let store = init_store(config.store, &heartbeat).await?;

        Ok(Self {
            worker: ArchiveWorker {
                archiver: Archiver {
                    versions: Arc::new(PgVersions {
                        url: config.database_url.clone(),
                    }),
                    store,
                    heartbeat: Arc::new(heartbeat),
                    tools: PgTools::new(config.pg_dump, config.pg_restore),
                    database_url: config.database_url.clone(),
                },
                interval: config.interval,
            },
        })
    }

    /// Run the worker until `shutdown` is cancelled.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        self.worker.run(shutdown).await;
        Ok(())
    }
}
