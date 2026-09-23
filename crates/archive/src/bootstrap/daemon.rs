//! Daemon assembly: connect the database, build the store and the heartbeat,
//! then run a dump at startup and every interval until asked to stop.

use std::{sync::Arc, time::Instant};

use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use object_store::aws::AmazonS3Builder;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use yog_bootstrap::{SHUTDOWN_GRACE, SecretUrl, shutdown_signal};
use yog_persistence::{Database, PgDatabaseInfo, ServerVersions};

use crate::{
    archiver::{Archiver, RunOutcome, VersionSource},
    bootstrap::{Config, config::StoreConfig},
    dump::{Connection, PgTools},
    heartbeat::HealthchecksHeartbeat,
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
        let store = build_store(&config.store).context("failed to configure the bucket")?;
        let heartbeat = HealthchecksHeartbeat::new(config.heartbeat_url)
            .context("failed to build the heartbeat client")?;
        let connection = Connection::from_secret(&config.database_url)?;

        Ok(Self {
            archiver: Archiver {
                versions: Arc::new(PgVersions {
                    url: config.database_url,
                }),
                store: Arc::new(store),
                heartbeat: Arc::new(heartbeat),
                tools: PgTools {
                    pg_dump: config.pg_dump,
                    pg_restore: config.pg_restore,
                },
                connection,
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

/// Reads the server's versions over a connection opened for this run and
/// closed after it.
///
/// Connecting once at startup made a refusing database — a wrong password, a
/// server that is down — stop the process before the heartbeat could say
/// anything. Under `restart: unless-stopped` that is a silent crash loop,
/// noticed only when the missing ping times out, hours later and without a
/// reason. Measured on 23 September 2026 with a wrong password: exit 1, no
/// signal. Connected here, the same refusal ends the run in `refused` and
/// sends the failure signal with the database's own words. One connection
/// every six hours costs nothing.
struct PgVersions {
    url: SecretUrl,
}

#[async_trait]
impl VersionSource for PgVersions {
    async fn server_versions(&self) -> Result<ServerVersions, String> {
        let database = Database::connect(self.url.expose()).await.map_err(|e| {
            format!(
                "cannot connect to the database: {}",
                self.url.scrub(&e.to_string())
            )
        })?;
        let versions = PgDatabaseInfo::new(database.pool().clone())
            .server_versions()
            .await
            .map_err(|e| e.to_string());
        database.pool().close().await;
        versions
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
