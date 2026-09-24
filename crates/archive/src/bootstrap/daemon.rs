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

    /// Dump now, then every interval, until `shutdown` is cancelled — the
    /// loop of every daemon here (`signals`' detectors, `context`'s workers):
    /// a ticker, and a stop that wins a tie.
    ///
    /// The first tick fires at once, which is the dump at startup. After each
    /// run the ticker is **reset**, so the next dump comes a full interval
    /// after this one *ended*: a run that outlasted the interval is not
    /// followed by another started back to back. No `MissedTickBehavior`
    /// gives that — even `Delay` fires an overdue tick at once and only
    /// spaces the ones after it.
    pub(crate) async fn run(self, shutdown: CancellationToken) -> anyhow::Result<()> {
        let mut ticker = tokio::time::interval(self.interval);
        loop {
            tokio::select! {
                // `biased`: a stop that arrives with a tick due must not start
                // a fresh dump.
                biased;

                () = shutdown.cancelled() => break,
                _ = ticker.tick() => {
                    if !self.run_once(&shutdown).await {
                        break;
                    }
                    ticker.reset();
                }
            }
        }
        info!("archiver stopped");
        Ok(())
    }

    /// One run, recorded and logged. `false` when a stop arrived and the run
    /// outlived [`SHUTDOWN_GRACE`]: the process leaves it behind, and the
    /// bucket's lifecycle rule removes its incomplete upload.
    ///
    /// A stop arriving mid-dump is seen by the run itself, which kills
    /// `pg_dump` and aborts the upload; the grace only bounds how long that
    /// may take.
    async fn run_once(&self, shutdown: &CancellationToken) -> bool {
        let started = Instant::now();
        let outcome = tokio::select! {
            outcome = self.archiver.run(Utc::now(), shutdown) => outcome,
            () = grace_expired(shutdown) => {
                warn!(grace_secs = SHUTDOWN_GRACE.as_secs(), "the run outlived the shutdown grace — leaving it");
                return false;
            }
        };
        metrics::record(&outcome, started.elapsed());
        log_outcome(&outcome, started.elapsed());
        true
    }
}

/// Resolves [`SHUTDOWN_GRACE`] after a stop is asked for, and never before.
async fn grace_expired(shutdown: &CancellationToken) {
    shutdown.cancelled().await;
    tokio::time::sleep(SHUTDOWN_GRACE).await;
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
