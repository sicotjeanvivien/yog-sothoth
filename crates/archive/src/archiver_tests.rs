//! One run of the archiver against fake `pg_dump` / `pg_restore` scripts, an
//! in-memory bucket and a heartbeat that remembers what it was told.
//!
//! Each failure case asserts the outcome **and** its reason, that the bucket
//! is left empty, that an upload it had opened was **aborted**, and that
//! exactly one failure signal was sent with that reason — a run that fails in
//! silence is the defect this crate exists to prevent.
//!
//! The abort is counted, not inferred from an empty bucket: an in-memory
//! upload that is neither completed nor aborted never shows up in a listing
//! either, so "the bucket is empty" stayed green with every `abort` removed.

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use chrono::TimeZone;
use futures_util::{StreamExt, stream::BoxStream};
use object_store::{
    GetOptions, GetResult, ListResult, MultipartUpload, ObjectMeta, ObjectStoreExt, PutOptions,
    PutPayload, PutResult, UploadPart, memory::InMemory,
};
use tempfile::TempDir;

use super::*;
use crate::infra::RecordingHeartbeat;

const VERSION_16: &str = "pg_dump (PostgreSQL) 16.14 (Debian 16.14-1.pgdg120+1)";
const ARCHIVE: &str = "PGDMP-fake-archive-body";
const PASSWORD: &str = "s3cret-password";

struct FixedVersions(Result<ServerVersions, String>);

#[async_trait]
impl VersionSource for FixedVersions {
    async fn server_versions(&self) -> Result<ServerVersions, String> {
        self.0.clone()
    }
}

fn server_16() -> FixedVersions {
    FixedVersions(Ok(ServerVersions {
        postgres_major: 16,
        timescaledb: "2.27.1".to_string(),
    }))
}

/// What happened to the multipart uploads a run opened.
#[derive(Debug, Default)]
struct Ledger {
    completed: AtomicUsize,
    aborted: AtomicUsize,
}

/// How the fake bucket answers an upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Accepting,
    /// Refuses to open any upload, as a key without the right would.
    Refusing,
}

/// An in-memory bucket that records how each upload ended.
#[derive(Debug)]
struct TestStore {
    inner: InMemory,
    ledger: Arc<Ledger>,
    bucket: Bucket,
}

impl std::fmt::Display for TestStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TestStore({:?})", self.bucket)
    }
}

#[derive(Debug)]
struct RecordingUpload {
    inner: Box<dyn MultipartUpload>,
    ledger: Arc<Ledger>,
}

#[async_trait]
impl MultipartUpload for RecordingUpload {
    fn put_part(&mut self, data: PutPayload) -> UploadPart {
        self.inner.put_part(data)
    }

    async fn complete(&mut self) -> object_store::Result<PutResult> {
        self.ledger.completed.fetch_add(1, Ordering::SeqCst);
        self.inner.complete().await
    }

    async fn abort(&mut self) -> object_store::Result<()> {
        self.ledger.aborted.fetch_add(1, Ordering::SeqCst);
        self.inner.abort().await
    }
}

#[async_trait]
impl ObjectStore for TestStore {
    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> object_store::Result<PutResult> {
        self.inner.put_opts(location, payload, opts).await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> object_store::Result<Box<dyn MultipartUpload>> {
        if self.bucket == Bucket::Refusing {
            return Err(object_store::Error::Generic {
                store: "TestStore",
                source: "AccessDenied: the bucket refused the upload".into(),
            });
        }
        let inner = self.inner.put_multipart_opts(location, opts).await?;
        Ok(Box::new(RecordingUpload {
            inner,
            ledger: Arc::clone(&self.ledger),
        }))
    }

    async fn get_opts(
        &self,
        location: &Path,
        options: GetOptions,
    ) -> object_store::Result<GetResult> {
        self.inner.get_opts(location, options).await
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, object_store::Result<Path>>,
    ) -> BoxStream<'static, object_store::Result<Path>> {
        self.inner.delete_stream(locations)
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, object_store::Result<ObjectMeta>> {
        self.inner.list(prefix)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> object_store::Result<ListResult> {
        self.inner.list_with_delimiter(prefix).await
    }

    async fn copy_opts(
        &self,
        from: &Path,
        to: &Path,
        options: object_store::CopyOptions,
    ) -> object_store::Result<()> {
        self.inner.copy_opts(from, to, options).await
    }
}

/// Serialises the tests that write scripts and run them.
///
/// Writing an executable and exec-ing it while another thread forks is the
/// classic `ETXTBSY` ("Text file busy") race: the forked child briefly holds
/// the file open for writing, and the exec fails. One test failed once in
/// about 80 runs, right after a rebuild, and could not be reproduced in 72
/// more — the race is the likely cause, not a confirmed one. Running these
/// tests one at a time removes the whole class for ~0.3 s.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A directory of fake client programs, and what the fake `pg_dump` saw.
/// Holds [`SERIAL`] for as long as the test lives.
struct Fakes {
    dir: TempDir,
    _serial: std::sync::MutexGuard<'static, ()>,
}

impl Fakes {
    fn new() -> Self {
        // A test that panics poisons the lock; the next one must still run.
        let serial = SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self {
            dir: tempfile::tempdir().unwrap(),
            _serial: serial,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Write an executable shell script. `$DIR` in `body` is the directory.
    fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path(name);
        let body = body.replace("$DIR", &self.dir.path().display().to_string());
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A `pg_dump` answering `--version` with `version`, and otherwise
    /// recording its arguments and password before running `dump`.
    fn pg_dump(&self, version: &str, dump: &str) -> PathBuf {
        self.script(
            "pg_dump",
            &format!(
                r#"if [ "$1" = "--version" ]; then echo "{version}"; exit 0; fi
printf '%s\n' "$@" > "$DIR/args"
printf '%s' "$PGPASSWORD" > "$DIR/password"
{dump}"#
            ),
        )
    }

    fn good_pg_dump(&self) -> PathBuf {
        self.pg_dump(VERSION_16, &format!("printf '{ARCHIVE}'"))
    }

    /// A `pg_restore` that reads the whole archive, records how it was
    /// asked to, and accepts it.
    fn good_pg_restore(&self) -> PathBuf {
        self.script(
            "pg_restore",
            r#"printf '%s\n' "$@" > "$DIR/restore-args"; cat > "$DIR/restore-input"; exit 0"#,
        )
    }

    fn read(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.path(name)).ok()
    }
}

struct Run {
    outcome: RunOutcome,
    signals: Vec<String>,
    store: Arc<dyn ObjectStore>,
    ledger: Arc<Ledger>,
}

impl Run {
    fn aborted(&self) -> usize {
        self.ledger.aborted.load(Ordering::SeqCst)
    }

    fn completed(&self) -> usize {
        self.ledger.completed.load(Ordering::SeqCst)
    }
}

const DATABASE_URL: &str = "postgresql://yog_archive:s3cret-password@db:5432/yog_sothoth";

async fn run_with(
    versions: FixedVersions,
    bucket: Bucket,
    pg_dump: PathBuf,
    pg_restore: PathBuf,
    cancel: CancellationToken,
) -> Run {
    run_full(versions, bucket, pg_dump, pg_restore, cancel, DATABASE_URL).await
}

async fn run_full(
    versions: FixedVersions,
    bucket: Bucket,
    pg_dump: PathBuf,
    pg_restore: PathBuf,
    cancel: CancellationToken,
    database_url: &str,
) -> Run {
    let ledger = Arc::new(Ledger::default());
    let store: Arc<dyn ObjectStore> = Arc::new(TestStore {
        inner: InMemory::new(),
        ledger: Arc::clone(&ledger),
        bucket,
    });
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let archiver = Archiver {
        versions: Arc::new(versions),
        store: Arc::clone(&store),
        heartbeat: heartbeat.clone(),
        tools: PgTools::new(pg_dump, pg_restore),
        database_url: yog_bootstrap::SecretUrl::for_tests(database_url),
    };
    let now = Utc.with_ymd_and_hms(2026, 9, 23, 6, 0, 0).unwrap();
    let outcome = archiver.run(now, &cancel).await;
    let signals = heartbeat.signals.lock().unwrap().clone();
    Run {
        outcome,
        signals,
        store,
        ledger,
    }
}

async fn run(pg_dump: PathBuf, pg_restore: PathBuf) -> Run {
    run_with(
        server_16(),
        Bucket::Accepting,
        pg_dump,
        pg_restore,
        CancellationToken::new(),
    )
    .await
}

async fn objects(store: &Arc<dyn ObjectStore>) -> Vec<String> {
    store
        .list(None)
        .map(|meta| meta.unwrap().location.to_string())
        .collect()
        .await
}

/// The run failed with `label`, its reason contains `reason`, nothing is in
/// the bucket, the upload was aborted `aborted` times and never completed,
/// and exactly one failure signal carried the label and the reason.
async fn assert_failed(run: &Run, label: &str, reason: &str, aborted: usize) {
    assert_eq!(run.outcome.label(), label, "{:?}", run.outcome);
    let text = format!("{:?}", run.outcome);
    assert!(text.contains(reason), "expected `{reason}` in {text}");
    assert_eq!(objects(&run.store).await, Vec::<String>::new());
    assert_eq!(run.completed(), 0, "a failed run completed its upload");
    assert_eq!(run.aborted(), aborted, "aborted uploads");
    assert_eq!(run.signals.len(), 1, "{:?}", run.signals);
    assert!(
        run.signals[0].starts_with(&format!("failure: {label}: ")),
        "{:?}",
        run.signals
    );
    assert!(run.signals[0].contains(reason), "{:?}", run.signals);
}

#[tokio::test]
async fn a_dump_is_archived_under_its_version_and_signalled_once() {
    let fakes = Fakes::new();
    let run = run(fakes.good_pg_dump(), fakes.good_pg_restore()).await;

    let key = "yog-sothoth/2026-09-23T060000Z_timescaledb-2.27.1.dump";
    assert_eq!(
        run.outcome,
        RunOutcome::Archived {
            key: key.to_string(),
            bytes: ARCHIVE.len() as u64
        }
    );
    assert_eq!(run.signals, vec!["success".to_string()]);
    assert_eq!((run.completed(), run.aborted()), (1, 0));

    let stored = run.store.get(&Path::from(key)).await.unwrap();
    let attributes = stored.attributes.clone();
    assert_eq!(stored.bytes().await.unwrap(), ARCHIVE.as_bytes());
    assert_eq!(
        attributes
            .get(&Attribute::Metadata("timescaledb-version".into()))
            .map(|v| v.as_ref().to_string()),
        Some("2.27.1".to_string())
    );

    // What pg_restore checked is the very archive stored, and it was asked
    // to write it out — which reads every data block — not merely to list
    // its table of contents, which passes a truncated dump.
    assert_eq!(fakes.read("restore-input").as_deref(), Some(ARCHIVE));
    assert_eq!(
        fakes.read("restore-args").as_deref(),
        Some("--file=/dev/null\n")
    );
}

#[tokio::test]
async fn the_password_reaches_pg_dump_through_its_environment_only() {
    let fakes = Fakes::new();
    run(fakes.good_pg_dump(), fakes.good_pg_restore()).await;

    let args = fakes.read("args").unwrap();
    assert!(
        !args.contains(PASSWORD),
        "password in the arguments: {args}"
    );
    assert!(args.contains("--format=custom"), "{args}");
    assert!(args.contains("--no-password"), "{args}");
    assert_eq!(fakes.read("password").as_deref(), Some(PASSWORD));
}

#[tokio::test]
async fn a_failing_pg_dump_leaves_no_dump_and_signals_why() {
    let fakes = Fakes::new();
    let pg_dump = fakes.pg_dump(
        VERSION_16,
        r#"printf 'PGDMP-partial'
echo 'pg_dump: error: connection to server failed: password authentication failed' >&2
exit 1"#,
    );
    let run = run(pg_dump, fakes.good_pg_restore()).await;

    assert_failed(&run, "dump_failed", "password authentication failed", 1).await;
}

#[tokio::test]
async fn an_archive_pg_restore_cannot_read_is_not_kept() {
    let fakes = Fakes::new();
    let pg_restore = fakes.script(
        "pg_restore",
        r#"cat > /dev/null
echo 'pg_restore: error: input file does not appear to be a valid archive' >&2
exit 1"#,
    );
    let run = run(fakes.good_pg_dump(), pg_restore).await;

    assert_failed(
        &run,
        "unreadable",
        "does not appear to be a valid archive",
        1,
    )
    .await;
}

#[tokio::test]
async fn a_bucket_that_refuses_the_upload_is_signalled() {
    let fakes = Fakes::new();
    let run = run_with(
        server_16(),
        Bucket::Refusing,
        fakes.good_pg_dump(),
        fakes.good_pg_restore(),
        CancellationToken::new(),
    )
    .await;

    assert_failed(&run, "store_failed", "AccessDenied", 0).await;
    assert_eq!(
        fakes.read("args"),
        None,
        "pg_dump ran with nowhere to write"
    );
}

#[tokio::test]
async fn a_pg_dump_of_another_major_is_refused_before_dumping() {
    let fakes = Fakes::new();
    let pg_dump = fakes.pg_dump(
        "pg_dump (PostgreSQL) 14.23 (Ubuntu 14.23-0ubuntu0.22.04.1)",
        &format!("printf '{ARCHIVE}'"),
    );
    let run = run(pg_dump, fakes.good_pg_restore()).await;

    assert_failed(
        &run,
        "refused",
        "pg_dump is PostgreSQL 14 and the server is PostgreSQL 16",
        0,
    )
    .await;
    assert_eq!(fakes.read("args"), None, "pg_dump ran despite the mismatch");
}

#[tokio::test]
async fn unreadable_server_versions_refuse_the_run() {
    let fakes = Fakes::new();
    let run = run_with(
        FixedVersions(Err("the timescaledb extension is not installed".to_string())),
        Bucket::Accepting,
        fakes.good_pg_dump(),
        fakes.good_pg_restore(),
        CancellationToken::new(),
    )
    .await;

    assert_failed(
        &run,
        "refused",
        "the timescaledb extension is not installed",
        0,
    )
    .await;
}

#[tokio::test]
async fn a_stop_mid_dump_kills_pg_dump_keeps_nothing_and_signals_nothing() {
    let fakes = Fakes::new();
    let pg_dump = fakes.pg_dump(VERSION_16, "printf 'PGDMP-start'\nexec sleep 30");
    let cancel = CancellationToken::new();
    let stopper = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        stopper.cancel();
    });

    let run = tokio::time::timeout(
        Duration::from_secs(10),
        run_with(
            server_16(),
            Bucket::Accepting,
            pg_dump,
            fakes.good_pg_restore(),
            cancel,
        ),
    )
    .await
    .expect("the stop must end the run, not wait for pg_dump");

    assert_eq!(run.outcome, RunOutcome::Cancelled);
    assert_eq!(run.signals, Vec::<String>::new());
    assert_eq!(objects(&run.store).await, Vec::<String>::new());
    assert_eq!((run.completed(), run.aborted()), (0, 1));
}

#[tokio::test]
async fn an_archive_larger_than_any_head_is_checked_whole() {
    // 6 MiB: past the 4 MiB head the first version checked, which the table
    // of contents would have outgrown as chunks accumulate.
    const SIZE: usize = 6 * 1024 * 1024;
    let fakes = Fakes::new();
    let pg_dump = fakes.pg_dump(VERSION_16, &format!("head -c {SIZE} /dev/zero"));
    let run = run(pg_dump, fakes.good_pg_restore()).await;

    assert_eq!(run.outcome.label(), "archived", "{:?}", run.outcome);
    let checked = fs::metadata(fakes.path("restore-input")).unwrap().len();
    assert_eq!(
        checked, SIZE as u64,
        "pg_restore must be fed the whole archive"
    );
}

#[tokio::test]
async fn a_pg_restore_that_stops_reading_early_is_judged_by_its_exit_status() {
    let fakes = Fakes::new();
    let pg_dump = fakes.pg_dump(VERSION_16, "head -c 2000000 /dev/zero");
    let pg_restore = fakes.script("pg_restore", "exit 0");
    let run = run(pg_dump, pg_restore).await;

    assert_eq!(run.outcome.label(), "archived", "{:?}", run.outcome);
}

#[tokio::test]
async fn a_database_url_pg_dump_cannot_use_is_refused_and_signalled() {
    let fakes = Fakes::new();
    let run = run_full(
        server_16(),
        Bucket::Accepting,
        fakes.good_pg_dump(),
        fakes.good_pg_restore(),
        CancellationToken::new(),
        "not a url",
    )
    .await;

    assert_failed(&run, "refused", "the database URL is not a valid URL", 0).await;
    assert_eq!(fakes.read("args"), None);
}
