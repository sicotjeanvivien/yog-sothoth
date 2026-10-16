//! The alarm against a scripted repository and a recording heartbeat; every case asserts the
//! exact signal, reason included: a wrong reason reads as green anywhere else.

use std::future::Future;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::TimeZone;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use yog_bootstrap::RecordingHeartbeat;
use yog_core::{RepositoryError, RepositoryResult};

use super::*;

fn at(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, second)
        .unwrap()
}

/// The instant every check below is made at.
fn now() -> DateTime<Utc> {
    at(12, 0, 0)
}

/// Hands back what it was given, at every call.
struct ScriptedRepository(Mutex<RepositoryResult<Option<DateTime<Utc>>>>);

#[async_trait]
impl EventFreshnessRepository for ScriptedRepository {
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        self.0.lock().unwrap().clone()
    }
}

/// Never answers.
struct HangingRepository;

#[async_trait]
impl EventFreshnessRepository for HangingRepository {
    async fn last_event_at(&self) -> RepositoryResult<Option<DateTime<Utc>>> {
        std::future::pending().await
    }
}

fn alarm_over(
    repository: Arc<dyn EventFreshnessRepository>,
) -> (IngestionAlarm, Arc<RecordingHeartbeat>) {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let alarm = IngestionAlarm::new(repository, Some(heartbeat.clone()));
    (alarm, heartbeat)
}

fn alarm(
    last_event_at: RepositoryResult<Option<DateTime<Utc>>>,
) -> (IngestionAlarm, Arc<RecordingHeartbeat>) {
    alarm_over(Arc::new(ScriptedRepository(Mutex::new(last_event_at))))
}

fn signals(heartbeat: &RecordingHeartbeat) -> Vec<String> {
    heartbeat.signals.lock().unwrap().clone()
}

#[tokio::test]
async fn a_recent_event_signals_success() {
    let (alarm, heartbeat) = alarm(Ok(Some(at(11, 59, 0))));

    let verdict = alarm.check(now()).await;

    assert_eq!(verdict, Verdict::Live);
    assert_eq!(signals(&heartbeat), ["success"]);
}

/// A lull is not a fault: the dashboard shows it orange, the check stays green.
#[tokio::test]
async fn a_delayed_ingestion_still_signals_success() {
    let (alarm, heartbeat) = alarm(Ok(Some(at(11, 50, 0))));

    let verdict = alarm.check(now()).await;

    assert_eq!(verdict, Verdict::Delayed);
    assert_eq!(signals(&heartbeat), ["success"]);
}

/// The two sides of the dashboard's fifteen minutes: a threshold of the alarm's own, even one
/// second off, turns one red.
#[tokio::test]
async fn fifteen_minutes_exactly_is_still_delayed() {
    let (alarm, heartbeat) = alarm(Ok(Some(at(11, 45, 0))));

    let verdict = alarm.check(now()).await;

    assert_eq!(verdict, Verdict::Delayed);
    assert_eq!(signals(&heartbeat), ["success"]);
}

#[tokio::test]
async fn one_second_past_fifteen_minutes_fails_the_check_with_the_age() {
    let (alarm, heartbeat) = alarm(Ok(Some(at(11, 44, 59))));

    let verdict = alarm.check(now()).await;

    assert_eq!(
        verdict,
        Verdict::Stale {
            last_event_at: Some(at(11, 44, 59))
        }
    );
    assert_eq!(
        signals(&heartbeat),
        ["failure: stale: last event 0h15m ago, at 2026-01-15 11:44 UTC"]
    );
}

/// Hours, not only minutes: the reason stays readable after a long outage.
#[tokio::test]
async fn a_long_outage_says_how_long_in_hours() {
    let (alarm, heartbeat) = alarm(Ok(Some(at(6, 48, 0))));

    alarm.check(now()).await;

    assert_eq!(
        signals(&heartbeat),
        ["failure: stale: last event 5h12m ago, at 2026-01-15 06:48 UTC"]
    );
}

/// An empty database is no evidence of a live flow.
#[tokio::test]
async fn no_event_ever_indexed_fails_the_check() {
    let (alarm, heartbeat) = alarm(Ok(None));

    let verdict = alarm.check(now()).await;

    assert_eq!(
        verdict,
        Verdict::Stale {
            last_event_at: None
        }
    );
    assert_eq!(
        signals(&heartbeat),
        ["failure: stale: no event has ever been indexed"]
    );
}

/// A database that refuses fails the check, saying what the database said.
#[tokio::test]
async fn an_unreadable_last_event_fails_the_check_with_the_error() {
    let (alarm, heartbeat) = alarm(Err(RepositoryError::Backend(
        "connection refused".to_string(),
    )));

    let verdict = alarm.check(now()).await;

    assert_eq!(verdict.label(), "unreadable");
    assert_eq!(
        signals(&heartbeat),
        ["failure: unreadable: repository backend failure: connection refused"]
    );
}

/// A read that hangs ends the check in `/fail` with the reason, not in silence.
#[tokio::test(start_paused = true)]
async fn a_read_that_never_answers_fails_the_check() {
    let (alarm, heartbeat) = alarm_over(Arc::new(HangingRepository));

    let verdict = alarm.check(now()).await;

    assert_eq!(verdict.label(), "unreadable");
    assert_eq!(
        signals(&heartbeat),
        ["failure: unreadable: no answer within 90 s"]
    );
}

/// Without a check to report to, the verdict is still reached; only the signal is skipped.
#[tokio::test]
async fn without_a_heartbeat_the_verdict_is_still_reached() {
    let alarm = IngestionAlarm::new(Arc::new(ScriptedRepository(Mutex::new(Ok(None)))), None);

    let verdict = alarm.check(now()).await;

    assert_eq!(verdict.label(), "stale");
}

/// A stop asked for before the first tick ends the loop with nothing read or signalled.
#[tokio::test]
async fn a_stop_ends_the_loop_without_a_check() {
    let (alarm, heartbeat) = alarm(Ok(None));
    let shutdown = CancellationToken::new();
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(5), alarm.run(shutdown))
        .await
        .expect("the alarm must stop when asked")
        .expect("Infallible");

    assert!(signals(&heartbeat).is_empty());
}

/// ⚠️ The check races the stop: awaited outside the race, the stop would wait out `READ_TIMEOUT`
/// and a ping, past Docker's ten seconds.
#[tokio::test(start_paused = true)]
async fn a_stop_during_a_check_ends_the_loop_without_waiting_for_it() {
    let (alarm, heartbeat) = alarm_over(Arc::new(HangingRepository));
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(alarm.run(shutdown.clone()));

    // The first tick fires at once; one second in, the read is hanging.
    tokio::time::sleep(Duration::from_secs(1)).await;
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("the stop must not wait for the check")
        .expect("the alarm task must not panic")
        .expect("Infallible");
    assert!(
        signals(&heartbeat).is_empty(),
        "a cut check signals nothing"
    );
}

/// Drive `body` on a current-thread runtime with a local recorder, which binds the current thread.
fn with_recorder(body: impl Future<Output = ()>) -> Snapshotter {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    ::metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(body);
    });
    snapshotter
}

/// The checks counter by outcome, from one snapshot: reading drains the counters.
fn checks_by_outcome(snapshotter: &Snapshotter) -> Vec<(String, DebugValue)> {
    let mut checks: Vec<(String, DebugValue)> = snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .filter(|(key, _, _, _)| key.key().name() == "yog_indexer_ingestion_checks_total")
        .map(|(key, _, _, value)| {
            let outcome = key
                .key()
                .labels()
                .find(|l| l.key() == "outcome")
                .map(|l| l.value().to_string())
                .unwrap_or_default();
            (outcome, value)
        })
        .collect();
    checks.sort_by(|a, b| a.0.cmp(&b.0));
    checks
}

/// Each check is counted under its own outcome, and under no other.
#[test]
fn every_check_is_counted_under_its_outcome() {
    let snapshotter = with_recorder(async {
        alarm(Ok(Some(at(11, 59, 0)))).0.check(now()).await;
        alarm(Ok(Some(at(11, 50, 0)))).0.check(now()).await;
        alarm(Ok(None)).0.check(now()).await;
        alarm(Ok(None)).0.check(now()).await;
        alarm(Err(RepositoryError::Backend("down".to_string())))
            .0
            .check(now())
            .await;
    });

    assert_eq!(
        checks_by_outcome(&snapshotter),
        [
            ("delayed".to_string(), DebugValue::Counter(1)),
            ("live".to_string(), DebugValue::Counter(1)),
            ("stale".to_string(), DebugValue::Counter(2)),
            ("unreadable".to_string(), DebugValue::Counter(1)),
        ]
    );
}
