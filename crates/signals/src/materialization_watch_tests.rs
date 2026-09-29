//! The watch against a scripted repository and a heartbeat that records what
//! it is told. Every case asserts the **exact** signal — the reason included —
//! because a check that went to `/fail` for the wrong reason, or to success
//! with an aggregate late, reads as green anywhere else.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::TimeZone;
use yog_bootstrap::RecordingHeartbeat;
use yog_core::{RepositoryError, RepositoryResult};

use super::*;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, 0).unwrap()
}

fn aggregate(name: &str, oldest_pending_at: Option<DateTime<Utc>>) -> AggregateMaterialization {
    AggregateMaterialization {
        aggregate: name.to_string(),
        watermark: Some(at(6, 0)),
        oldest_pending_at,
    }
}

/// Hands back what it was given, once per call.
struct ScriptedRepository(Mutex<RepositoryResult<Vec<AggregateMaterialization>>>);

#[async_trait]
impl MaterializationRepository for ScriptedRepository {
    async fn progress(&self) -> RepositoryResult<Vec<AggregateMaterialization>> {
        self.0.lock().unwrap().clone()
    }
}

fn watch(
    progress: RepositoryResult<Vec<AggregateMaterialization>>,
) -> (MaterializationWatch, Arc<RecordingHeartbeat>) {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let watch = MaterializationWatch::new(
        Arc::new(ScriptedRepository(Mutex::new(progress))),
        Some(heartbeat.clone()),
        MaterializationWatchSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );
    (watch, heartbeat)
}

fn signals(heartbeat: &RecordingHeartbeat) -> Vec<String> {
    heartbeat.signals.lock().unwrap().clone()
}

#[tokio::test]
async fn every_aggregate_within_the_limit_signals_success() {
    let (watch, heartbeat) = watch(Ok(vec![
        aggregate("swaps_hourly", Some(at(9, 0))),
        aggregate("claims_hourly", None),
    ]));

    let verdict = watch.check(at(12, 0)).await;

    assert_eq!(verdict, Verdict::OnTime);
    assert_eq!(signals(&heartbeat), ["success"]);
}

/// The late aggregate is named with its wait and the limit; the one on time
/// and the one at rest are not named at all.
#[tokio::test]
async fn a_late_aggregate_fails_the_check_by_name() {
    let (watch, heartbeat) = watch(Ok(vec![
        aggregate("swaps_hourly", Some(at(6, 48))),
        aggregate("liquidity_hourly", Some(at(10, 0))),
        aggregate("claims_hourly", None),
    ]));

    watch.check(at(12, 0)).await;

    assert_eq!(
        signals(&heartbeat),
        ["failure: late (limit 4h00m): swaps_hourly pending for 5h12m"]
    );
}

/// A database that refuses is not a quiet tick: the check fails, and says
/// what the database said.
#[tokio::test]
async fn an_unreadable_progress_fails_the_check_with_the_error() {
    let (watch, heartbeat) = watch(Err(RepositoryError::Backend(
        "connection refused".to_string(),
    )));

    let verdict = watch.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "unreadable");
    assert_eq!(
        signals(&heartbeat),
        ["failure: unreadable: repository backend failure: connection refused"]
    );
}

/// Without a check to report to — development — the verdict is still reached;
/// only the signal is skipped.
#[tokio::test]
async fn without_a_heartbeat_the_verdict_is_still_reached() {
    let watch = MaterializationWatch::new(
        Arc::new(ScriptedRepository(Mutex::new(Ok(vec![aggregate(
            "swaps_hourly",
            Some(at(1, 0)),
        )])))),
        None,
        MaterializationWatchSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );

    let verdict = watch.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "late");
}

/// A stop asked for before the first tick ends the loop without a check —
/// nothing is read, nothing is signalled.
#[tokio::test]
async fn a_stop_ends_the_loop_without_a_check() {
    let (watch, heartbeat) = watch(Ok(vec![aggregate("swaps_hourly", None)]));
    let shutdown = CancellationToken::new();
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(5), watch.run(shutdown))
        .await
        .expect("the watch must stop when asked");

    assert!(signals(&heartbeat).is_empty());
}

/// Never answers.
struct HangingRepository;

#[async_trait]
impl MaterializationRepository for HangingRepository {
    async fn progress(&self) -> RepositoryResult<Vec<AggregateMaterialization>> {
        std::future::pending().await
    }
}

/// A read that hangs ends the check in `/fail`, with the reason — not in a
/// silence the check would read as a stopped daemon.
#[tokio::test(start_paused = true)]
async fn a_read_that_never_answers_fails_the_check() {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let watch = MaterializationWatch::new(
        Arc::new(HangingRepository),
        Some(heartbeat.clone()),
        MaterializationWatchSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );

    let verdict = watch.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "unreadable");
    assert_eq!(
        signals(&heartbeat),
        ["failure: unreadable: no answer within 60 s"]
    );
}
