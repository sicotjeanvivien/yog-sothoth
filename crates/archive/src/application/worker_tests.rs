//! The loop against a run whose length the test decides: a version source
//! that sleeps, in tokio's paused time, then refuses. No `pg_dump` ever
//! starts — a refused run ends before one could — so these tests measure the
//! loop alone: when a run starts, and when the loop lets go of one.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use object_store::memory::InMemory;
use tokio::time::Instant;
use yog_bootstrap::{RecordingHeartbeat, SecretUrl};
use yog_persistence::{PgTools, ServerVersions};

use super::*;
use crate::application::VersionSource;

/// A version read that takes `takes`, then refuses — and remembers when each
/// run started.
struct SlowRefusal {
    takes: Duration,
    starts: Arc<Mutex<Vec<Instant>>>,
}

#[async_trait]
impl VersionSource for SlowRefusal {
    async fn server_versions(&self) -> Result<ServerVersions, String> {
        self.starts.lock().unwrap().push(Instant::now());
        tokio::time::sleep(self.takes).await;
        Err("refused on purpose".to_string())
    }
}

/// A worker whose every run takes `takes`, and the start times it records.
fn worker(interval: Duration, takes: Duration) -> (ArchiveWorker, Arc<Mutex<Vec<Instant>>>) {
    let starts = Arc::new(Mutex::new(Vec::new()));
    let worker = ArchiveWorker {
        archiver: Archiver {
            versions: Arc::new(SlowRefusal {
                takes,
                starts: Arc::clone(&starts),
            }),
            store: Arc::new(InMemory::new()),
            heartbeat: Arc::new(RecordingHeartbeat::default()),
            tools: PgTools::new("pg_dump".into(), "pg_restore".into()),
            database_url: SecretUrl::for_tests("postgresql://yog_archive@db:5432/yog_sothoth"),
        },
        interval,
    };
    (worker, starts)
}

/// A run longer than the interval is followed by a full interval, not by an
/// overdue tick fired at once: the next dump starts `interval` after this
/// one *ended*. Without `reset`, the second run would start at 15 s.
#[tokio::test(start_paused = true)]
async fn the_next_run_starts_a_full_interval_after_the_last_one_ended() {
    let (worker, starts) = worker(Duration::from_secs(10), Duration::from_secs(15));
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker.run(shutdown.clone()));

    tokio::time::sleep(Duration::from_secs(45)).await;
    shutdown.cancel();
    running.await.unwrap();

    let starts = starts.lock().unwrap().clone();
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert_eq!(starts[1] - starts[0], Duration::from_secs(25));
}

/// A stop already asked for when the first tick is due starts no dump. The
/// first tick is ready at once, so without `biased` the `select!` would pick
/// it about one time in two: repeated, the test cannot pass by chance.
#[tokio::test(start_paused = true)]
async fn a_stop_that_ties_with_a_due_tick_starts_no_run() {
    for _ in 0..32 {
        let (worker, starts) = worker(Duration::from_secs(10), Duration::ZERO);
        let shutdown = CancellationToken::new();
        shutdown.cancel();

        worker.run(shutdown).await;

        assert!(starts.lock().unwrap().is_empty());
    }
}

/// A run that outlives the grace after a stop is left behind: the loop ends
/// at the grace, not when the run would have. The version read here ignores
/// the stop, as a stuck one would.
#[tokio::test(start_paused = true)]
async fn a_run_that_outlives_the_grace_is_left_behind() {
    let (worker, starts) = worker(Duration::from_secs(10), Duration::from_secs(3600));
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(worker.run(shutdown.clone()));

    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(starts.lock().unwrap().len(), 1, "the run is under way");
    let stopped_at = Instant::now();
    shutdown.cancel();

    tokio::time::timeout(SHUTDOWN_GRACE + Duration::from_secs(1), running)
        .await
        .expect("the loop must let go at the grace")
        .unwrap();
    assert_eq!(stopped_at.elapsed(), SHUTDOWN_GRACE);
}
