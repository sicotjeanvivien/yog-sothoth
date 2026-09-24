//! One archiving run: read the server's versions, dump, stream to the bucket,
//! check the archive, signal.
//!
//! A run always ends in a [`RunOutcome`], and the heartbeat is decided from
//! it in one `match` with no catch-all: a new way to end cannot be added
//! without deciding what it signals. That is the whole defence against the
//! failure that matters here — a backup that stops working and says nothing.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use object_store::{
    Attribute, Attributes, ObjectStore, PutMultipartOptions, WriteMultipart, path::Path,
};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use yog_bootstrap::SecretUrl;
use yog_persistence::{PgTools, ServerVersions};

use crate::infra::Heartbeat;

mod run_outcome;
mod stream;

pub(crate) use run_outcome::{RunFailure, RunOutcome};
use stream::stream;

/// Where the dumps sit in the bucket, which may one day hold other projects'.
const KEY_PREFIX: &str = "yog-sothoth/";

/// Size of one multipart part. S3 requires at least 5 MiB for every part
/// but the last; 8 MiB keeps the memory an upload holds small.
const PART_SIZE: usize = 8 * 1024 * 1024;

/// The server facts a dump depends on. A trait so the tests need no
/// database; the production implementation connects for each run
/// (`infra::versions`), so that a database that refuses ends a run in
/// `refused` — signalled — instead of stopping the process before it can
/// signal anything.
#[async_trait]
pub(crate) trait VersionSource: Send + Sync {
    /// The error is the whole reason sent with the failure signal, saying
    /// that it is the versions that could not be read.
    async fn server_versions(&self) -> Result<ServerVersions, String>;
}

pub(crate) struct Archiver {
    pub(crate) versions: Arc<dyn VersionSource>,
    pub(crate) store: Arc<dyn ObjectStore>,
    pub(crate) heartbeat: Arc<dyn Heartbeat>,
    pub(crate) tools: PgTools,
    /// Handed to `pg_dump` at each run, so that a URL it cannot use ends a run
    /// in a signalled failure instead of stopping the process at startup.
    pub(crate) database_url: SecretUrl,
}

impl Archiver {
    /// Run once, signal the outcome, and return it.
    pub(crate) async fn run(&self, now: DateTime<Utc>, cancel: &CancellationToken) -> RunOutcome {
        let outcome = self.archive(now, cancel).await;
        match &outcome {
            RunOutcome::Archived { .. } => self.heartbeat.success().await,
            RunOutcome::Cancelled => {}
            RunOutcome::Failed(failure) => {
                self.heartbeat
                    .failure(&format!("{}: {}", failure.kind.label(), failure.reason))
                    .await;
            }
        }
        outcome
    }

    async fn archive(&self, now: DateTime<Utc>, cancel: &CancellationToken) -> RunOutcome {
        match self.try_archive(now, cancel).await {
            Ok((key, bytes)) => RunOutcome::Archived { key, bytes },
            Err(Interrupted::Cancelled) => RunOutcome::Cancelled,
            Err(Interrupted::Failed(failure)) => RunOutcome::Failed(failure),
        }
    }

    /// The run in two phases. Up to the upload, nothing is running and a
    /// failure just returns. Once the upload is open, [`stream()`] owns
    /// `pg_dump` and `pg_restore`: when it fails, both are dropped — killed —
    /// as it returns, and only then is the upload aborted, so `pg_dump` never
    /// holds its connection for the length of a round-trip to the bucket.
    async fn try_archive(
        &self,
        now: DateTime<Utc>,
        cancel: &CancellationToken,
    ) -> Result<(String, u64), Interrupted> {
        let versions = self
            .versions
            .server_versions()
            .await
            .map_err(RunFailure::refused)?;
        self.tools
            .ensure_matches(&versions)
            .await
            .map_err(RunFailure::refused)?;

        let key = object_key(now, &versions.timescaledb);
        let options = PutMultipartOptions {
            attributes: Attributes::from_iter([
                (
                    Attribute::Metadata("timescaledb-version".into()),
                    versions.timescaledb.clone(),
                ),
                (
                    Attribute::Metadata("postgres-major".into()),
                    versions.postgres_major.to_string(),
                ),
            ]),
            ..Default::default()
        };
        let upload = self
            .store
            .put_multipart_opts(&Path::from(key.as_str()), options)
            .await
            .map_err(|e| RunFailure::store_failed(format!("cannot start the upload: {e}")))?;
        let mut writer = WriteMultipart::new_with_chunk_size(upload, PART_SIZE);

        let bytes = match stream(&self.tools, &self.database_url, &mut writer, cancel).await {
            Ok(bytes) => bytes,
            Err(interrupted) => {
                abort(writer).await;
                return Err(interrupted);
            }
        };
        writer
            .finish()
            .await
            .map_err(|e| RunFailure::store_failed(format!("cannot complete the upload: {e}")))?;
        Ok((key, bytes))
    }
}

/// Why a run stopped before archiving. Internal to the run: a stop is not a
/// failure, and [`Archiver::archive`] turns each into its [`RunOutcome`].
enum Interrupted {
    Cancelled,
    Failed(RunFailure),
}

impl From<RunFailure> for Interrupted {
    fn from(failure: RunFailure) -> Self {
        Self::Failed(failure)
    }
}

/// `yog-sothoth/2026-09-23T060000Z_timescaledb-2.27.1.dump`: sortable by
/// time, and naming the TimescaleDB version the dump restores into — the
/// restore procedure requires that exact version, and the object's name is
/// what an operator reads first. The version is also in the object's metadata.
pub(crate) fn object_key(now: DateTime<Utc>, timescaledb: &str) -> String {
    format!(
        "{KEY_PREFIX}{}_timescaledb-{timescaledb}.dump",
        now.format("%Y-%m-%dT%H%M%SZ")
    )
}

/// Abandon an upload, so its parts do not linger in the bucket.
///
/// A failure here is logged, not propagated: the run already has its
/// outcome, and the bucket's lifecycle rule removes incomplete uploads.
async fn abort(writer: WriteMultipart) {
    if let Err(e) = writer.abort().await {
        warn!(error = %e, "cannot abort the multipart upload — the bucket's lifecycle rule will remove its parts");
    }
}

#[cfg(test)]
#[path = "archiver_tests.rs"]
mod tests;
