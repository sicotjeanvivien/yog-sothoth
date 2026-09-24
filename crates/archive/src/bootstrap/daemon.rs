//! Daemon assembly: build the store, the heartbeat and the archiver, then run
//! a dump at startup and every interval until asked to stop.

use std::{sync::Arc, time::Instant};

use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use yog_bootstrap::SHUTDOWN_GRACE;
use yog_persistence::PgTools;

use crate::{
    archiver::{Archiver, RunOutcome},
    bootstrap::Config,
    infra::PgVersions,
    metrics,
};

mod init;

use init::{init_heartbeat, init_store};

pub(crate) struct Daemon {
    archiver: Archiver,
    interval: std::time::Duration,
}

impl Daemon {
    /// Build everything a run needs. Nothing here touches the database: see
    /// [`PgVersions`] for why the connection belongs to the run.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        let heartbeat = init_heartbeat(config.heartbeat_url)?;
        let store = init_store(&config.store, &heartbeat).await?;

        Ok(Self {
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
        })
    }

    /// Dump now, then every interval, until `shutdown` is cancelled.
    ///
    /// A stop arriving mid-dump kills `pg_dump` and aborts the upload; if
    /// that takes longer than [`SHUTDOWN_GRACE`], the process leaves anyway
    /// and the bucket's lifecycle rule removes the incomplete upload.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        loop {
            let started = Instant::now();
            let run = self.archiver.run(Utc::now(), &shutdown);
            tokio::pin!(run);
            let outcome = tokio::select! {
                outcome = &mut run => outcome,
                () = async {
                    shutdown.cancelled().await;
                    tokio::time::sleep(SHUTDOWN_GRACE).await;
                } => {
                    warn!(grace_secs = SHUTDOWN_GRACE.as_secs(), "the run outlived the shutdown grace — leaving it");
                    return Ok(());
                }
            };
            metrics::record(&outcome, started.elapsed());
            log_outcome(&outcome, started.elapsed());

            if shutdown.is_cancelled() {
                break;
            }
            tokio::select! {
                () = shutdown.cancelled() => break,
                () = tokio::time::sleep(self.interval) => {}
            }
        }
        info!("archiver stopped");
        Ok(())
    }
}

fn log_outcome(outcome: &RunOutcome, elapsed: std::time::Duration) {
    let secs = elapsed.as_secs_f64();
    match outcome {
        RunOutcome::Archived { key, bytes } => {
            info!(key, bytes, secs, "dump archived");
        }
        RunOutcome::Cancelled => info!(secs, "run cancelled by the stop"),
        RunOutcome::Refused(reason)
        | RunOutcome::DumpFailed(reason)
        | RunOutcome::Unreadable(reason)
        | RunOutcome::StoreFailed(reason) => {
            error!(
                outcome = outcome.label(),
                reason, secs, "archiving run failed"
            );
        }
    }
}
