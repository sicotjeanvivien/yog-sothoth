//! Daemon assembly: build the store, the heartbeat and the archiver, then run
//! a dump at startup and every interval until asked to stop.

use std::{sync::Arc, time::Instant};

use anyhow::Context;
use chrono::Utc;
use object_store::aws::AmazonS3Builder;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use yog_bootstrap::{SHUTDOWN_GRACE, shutdown_signal};
use yog_persistence::PgTools;

use crate::{
    archiver::{Archiver, RunOutcome},
    bootstrap::{Config, config::StoreConfig},
    infra::{HealthchecksHeartbeat, Heartbeat, PgVersions},
    metrics,
};

pub(crate) struct Daemon {
    archiver: Archiver,
    interval: std::time::Duration,
}

impl Daemon {
    /// Build everything a run needs. Nothing here touches the database: see
    /// [`PgVersions`] for why the connection belongs to the run.
    pub(crate) async fn new(config: Config) -> anyhow::Result<Self> {
        // The heartbeat first: from here on, whatever fails can be told.
        let heartbeat = HealthchecksHeartbeat::new(config.heartbeat_url)
            .context("failed to build the heartbeat client")?;
        let store = match build_store(&config.store) {
            Ok(store) => store,
            Err(e) => {
                heartbeat
                    .failure(&format!("store_misconfigured: {e}"))
                    .await;
                return Err(anyhow::Error::new(e).context("failed to configure the bucket"));
            }
        };

        Ok(Self {
            archiver: Archiver {
                versions: Arc::new(PgVersions {
                    url: config.database_url.clone(),
                }),
                store: Arc::new(store),
                heartbeat: Arc::new(heartbeat),
                tools: PgTools::new(config.pg_dump, config.pg_restore),
                database_url: config.database_url.clone(),
            },
            interval: config.interval,
        })
    }

    /// Dump now, then every interval, until SIGTERM or Ctrl-C.
    ///
    /// A stop arriving mid-dump kills `pg_dump` and aborts the upload; if
    /// that takes longer than [`SHUTDOWN_GRACE`], the process leaves anyway
    /// and the bucket's lifecycle rule removes the incomplete upload.
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let shutdown = CancellationToken::new();
        let signal = shutdown.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            signal.cancel();
        });

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

/// The S3-compatible store. Path-style requests, which both Scaleway and a
/// local MinIO accept; plain HTTP only when the endpoint says `http://`.
fn build_store(config: &StoreConfig) -> object_store::Result<object_store::aws::AmazonS3> {
    AmazonS3Builder::new()
        .with_endpoint(&config.url)
        .with_allow_http(config.url.starts_with("http://"))
        .with_virtual_hosted_style_request(false)
        .with_bucket_name(&config.bucket)
        .with_region(&config.region)
        .with_access_key_id(config.access_key.expose())
        .with_secret_access_key(config.secret_key.expose())
        .build()
}
