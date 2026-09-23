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
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use yog_persistence::{PgDatabaseInfo, ServerVersions};

use crate::{
    dump::{Connection, PgTools, read_tail},
    heartbeat::Heartbeat,
};

/// Where the dumps sit in the bucket, which may one day hold other projects'.
const KEY_PREFIX: &str = "yog-sothoth/";

/// Size of one multipart part. S3 requires at least 5 MiB for every part
/// but the last; 8 MiB keeps the memory an upload holds small.
const PART_SIZE: usize = 8 * 1024 * 1024;

/// Parts uploaded concurrently before the reader waits. With [`PART_SIZE`],
/// the upload holds at most ~24 MiB (two in flight, one filling).
const PARTS_IN_FLIGHT: usize = 2;

/// How much of the start of the archive is kept for the readability check.
/// The table of contents is at the start and weighs a few hundred KiB for
/// this schema; see [`PgTools::check_readable`].
const HEAD_PROBE: usize = 4 * 1024 * 1024;

/// How a run ended. Every variant but `Archived` and `Cancelled` carries the
/// reason sent with the failure signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RunOutcome {
    /// A dump is in the bucket under `key`.
    Archived { key: String, bytes: u64 },
    /// No dump was attempted: the versions could not be read, or `pg_dump`
    /// is not the server's major.
    Refused(String),
    /// `pg_dump` could not start, failed, or its output could not be read.
    DumpFailed(String),
    /// `pg_dump` succeeded but `pg_restore` cannot read what it produced.
    Unreadable(String),
    /// The bucket refused the upload, or its completion.
    StoreFailed(String),
    /// The process is stopping. Not a failure, and not signalled: the next
    /// start dumps at once.
    Cancelled,
}

impl RunOutcome {
    /// The label used in metrics and in the failure signal.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Archived { .. } => "archived",
            Self::Refused(_) => "refused",
            Self::DumpFailed(_) => "dump_failed",
            Self::Unreadable(_) => "unreadable",
            Self::StoreFailed(_) => "store_failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// The server facts a dump depends on. A trait so the tests need no
/// database; the production implementation is [`PgDatabaseInfo`].
#[async_trait]
pub(crate) trait VersionSource: Send + Sync {
    async fn server_versions(&self) -> Result<ServerVersions, String>;
}

#[async_trait]
impl VersionSource for PgDatabaseInfo {
    async fn server_versions(&self) -> Result<ServerVersions, String> {
        PgDatabaseInfo::server_versions(self)
            .await
            .map_err(|e| e.to_string())
    }
}

pub(crate) struct Archiver {
    pub(crate) versions: Arc<dyn VersionSource>,
    pub(crate) store: Arc<dyn ObjectStore>,
    pub(crate) heartbeat: Arc<dyn Heartbeat>,
    pub(crate) tools: PgTools,
    pub(crate) connection: Connection,
}

impl Archiver {
    /// Run once, signal the outcome, and return it.
    pub(crate) async fn run(&self, now: DateTime<Utc>, cancel: &CancellationToken) -> RunOutcome {
        let outcome = self.archive(now, cancel).await;
        match &outcome {
            RunOutcome::Archived { .. } => self.heartbeat.success().await,
            RunOutcome::Cancelled => {}
            RunOutcome::Refused(reason)
            | RunOutcome::DumpFailed(reason)
            | RunOutcome::Unreadable(reason)
            | RunOutcome::StoreFailed(reason) => {
                self.heartbeat
                    .failure(&format!("{}: {reason}", outcome.label()))
                    .await;
            }
        }
        outcome
    }

    async fn archive(&self, now: DateTime<Utc>, cancel: &CancellationToken) -> RunOutcome {
        let versions = match self.versions.server_versions().await {
            Ok(versions) => versions,
            Err(e) => {
                return RunOutcome::Refused(format!("cannot read the server's versions: {e}"));
            }
        };
        let client_major = match self.tools.pg_dump_major().await {
            Ok(major) => major,
            Err(e) => return RunOutcome::Refused(e.to_string()),
        };
        if client_major != versions.postgres_major {
            return RunOutcome::Refused(format!(
                "pg_dump is PostgreSQL {client_major} and the server is PostgreSQL {}: \
                 the dump must be taken by the server's major",
                versions.postgres_major
            ));
        }

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
        let upload = match self
            .store
            .put_multipart_opts(&Path::from(key.as_str()), options)
            .await
        {
            Ok(upload) => upload,
            Err(e) => return RunOutcome::StoreFailed(format!("cannot start the upload: {e}")),
        };
        let mut writer = WriteMultipart::new_with_chunk_size(upload, PART_SIZE);

        let mut child = match self.tools.spawn_dump(&self.connection) {
            Ok(child) => child,
            Err(e) => {
                abort(writer).await;
                return RunOutcome::DumpFailed(e.to_string());
            }
        };
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let stderr = tokio::spawn(read_tail(child.stderr.take().expect("stderr is piped")));

        let mut head = Vec::new();
        let mut bytes: u64 = 0;
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let read = tokio::select! {
                biased;
                () = cancel.cancelled() => {
                    let _ = child.kill().await;
                    abort(writer).await;
                    return RunOutcome::Cancelled;
                }
                read = stdout.read(&mut buf) => read,
            };
            let n = match read {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) => {
                    let _ = child.kill().await;
                    abort(writer).await;
                    return RunOutcome::DumpFailed(format!("cannot read pg_dump's output: {e}"));
                }
            };
            if head.len() < HEAD_PROBE {
                let keep = (HEAD_PROBE - head.len()).min(n);
                head.extend_from_slice(&buf[..keep]);
            }
            if let Err(e) = writer.wait_for_capacity(PARTS_IN_FLIGHT).await {
                let _ = child.kill().await;
                abort(writer).await;
                return RunOutcome::StoreFailed(format!("a part was refused: {e}"));
            }
            writer.write(&buf[..n]);
            bytes += n as u64;
        }

        let status = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                let _ = child.kill().await;
                abort(writer).await;
                return RunOutcome::Cancelled;
            }
            status = child.wait() => status,
        };
        let stderr = stderr.await.unwrap_or_default();
        match status {
            Ok(status) if status.success() => {}
            Ok(status) => {
                abort(writer).await;
                return RunOutcome::DumpFailed(format!("pg_dump exited with {status}: {stderr}"));
            }
            Err(e) => {
                abort(writer).await;
                return RunOutcome::DumpFailed(format!("cannot wait for pg_dump: {e}"));
            }
        }

        if let Err(reason) = self.tools.check_readable(&head).await {
            abort(writer).await;
            return RunOutcome::Unreadable(reason);
        }

        match writer.finish().await {
            Ok(_) => RunOutcome::Archived { key, bytes },
            Err(e) => RunOutcome::StoreFailed(format!("cannot complete the upload: {e}")),
        }
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
