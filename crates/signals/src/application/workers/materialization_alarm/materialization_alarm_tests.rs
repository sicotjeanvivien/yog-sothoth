//! The alarm against a scripted repository and a recording heartbeat; every case asserts the
//! exact signal, reason included: a wrong reason reads as green anywhere else.

use std::sync::Mutex;

use async_trait::async_trait;
use chrono::TimeZone;
use yog_bootstrap::RecordingHeartbeat;
use yog_core::{RepositoryError, RepositoryResult, domain::MaterializationBacklog};

use super::*;
use crate::application::metrics_probe::{gauge, snapshot};

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, 0).unwrap()
}

fn aggregate(name: &str, oldest_pending_at: Option<DateTime<Utc>>) -> MaterializationBacklog {
    MaterializationBacklog {
        aggregate: name.to_string(),
        watermark: Some(at(6, 0)),
        oldest_pending_at,
    }
}

/// Hands back what it was given, once per call.
struct ScriptedRepository(Mutex<RepositoryResult<Vec<MaterializationBacklog>>>);

#[async_trait]
impl MaterializationBacklogRepository for ScriptedRepository {
    async fn backlogs(&self) -> RepositoryResult<Vec<MaterializationBacklog>> {
        self.0.lock().unwrap().clone()
    }
}

fn alarm(
    backlogs: RepositoryResult<Vec<MaterializationBacklog>>,
) -> (MaterializationAlarm, Arc<RecordingHeartbeat>) {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let alarm = MaterializationAlarm::new(
        Arc::new(ScriptedRepository(Mutex::new(backlogs))),
        Some(heartbeat.clone()),
        MaterializationAlarmSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );
    (alarm, heartbeat)
}

fn signals(heartbeat: &RecordingHeartbeat) -> Vec<String> {
    heartbeat.signals.lock().unwrap().clone()
}

#[tokio::test]
async fn every_aggregate_within_the_limit_signals_success() {
    let (alarm, heartbeat) = alarm(Ok(vec![
        aggregate("swaps_hourly", Some(at(9, 0))),
        aggregate("claims_hourly", None),
    ]));

    let verdict = alarm.check(at(12, 0)).await;

    assert_eq!(verdict, Verdict::OnTime);
    assert_eq!(signals(&heartbeat), ["success"]);
}

/// The late aggregate is named with its wait, where its materialisation ends,
/// and the limit; the one on time and the one at rest are not named at all.
#[tokio::test]
async fn a_late_aggregate_fails_the_check_by_name() {
    let (alarm, heartbeat) = alarm(Ok(vec![
        aggregate("swaps_hourly", Some(at(6, 48))),
        aggregate("liquidity_hourly", Some(at(10, 0))),
        aggregate("claims_hourly", None),
    ]));

    alarm.check(at(12, 0)).await;

    assert_eq!(
        signals(&heartbeat),
        [
            "failure: late (limit 4h00m): swaps_hourly pending for 5h12m, materialised up to 2026-01-15 06:00 UTC"
        ]
    );
}

/// An aggregate that never materialised says so, not a date, which would send the operator after
/// a refresh that stopped rather than one that never ran.
#[tokio::test]
async fn a_late_aggregate_never_materialised_says_so() {
    let never = MaterializationBacklog {
        watermark: None,
        ..aggregate("claims_hourly", Some(at(6, 48)))
    };
    let (alarm, heartbeat) = alarm(Ok(vec![never]));

    alarm.check(at(12, 0)).await;

    assert_eq!(
        signals(&heartbeat),
        ["failure: late (limit 4h00m): claims_hourly pending for 5h12m, never materialised"]
    );
}

/// A database that refuses fails the check, saying what the database said.
#[tokio::test]
async fn unreadable_backlogs_fail_the_check_with_the_error() {
    let (alarm, heartbeat) = alarm(Err(RepositoryError::Backend(
        "connection refused".to_string(),
    )));

    let verdict = alarm.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "unreadable");
    assert_eq!(
        signals(&heartbeat),
        ["failure: unreadable: repository backend failure: connection refused"]
    );
}

/// Without a check to report to, the verdict is still reached; only the signal is skipped.
#[tokio::test]
async fn without_a_heartbeat_the_verdict_is_still_reached() {
    let alarm = MaterializationAlarm::new(
        Arc::new(ScriptedRepository(Mutex::new(Ok(vec![aggregate(
            "swaps_hourly",
            Some(at(1, 0)),
        )])))),
        None,
        MaterializationAlarmSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );

    let verdict = alarm.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "late");
}

/// A stop asked for before the first tick ends the loop with nothing read or signalled.
#[tokio::test]
async fn a_stop_ends_the_loop_without_a_check() {
    let (alarm, heartbeat) = alarm(Ok(vec![aggregate("swaps_hourly", None)]));
    let shutdown = CancellationToken::new();
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(5), alarm.run(shutdown))
        .await
        .expect("the alarm must stop when asked");

    assert!(signals(&heartbeat).is_empty());
}

/// Never answers.
struct HangingRepository;

#[async_trait]
impl MaterializationBacklogRepository for HangingRepository {
    async fn backlogs(&self) -> RepositoryResult<Vec<MaterializationBacklog>> {
        std::future::pending().await
    }
}

/// A read that hangs ends the check in `/fail` with the reason, not in silence.
#[tokio::test(start_paused = true)]
async fn a_read_that_never_answers_fails_the_check() {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let alarm = MaterializationAlarm::new(
        Arc::new(HangingRepository),
        Some(heartbeat.clone()),
        MaterializationAlarmSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );

    let verdict = alarm.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "unreadable");
    assert_eq!(
        signals(&heartbeat),
        ["failure: unreadable: no answer within 90 s"]
    );
}

/// An empty answer means the aggregates are no longer found, not that all are on time.
#[tokio::test]
async fn no_aggregate_reported_fails_the_check() {
    let (alarm, heartbeat) = alarm(Ok(Vec::new()));

    let verdict = alarm.check(at(12, 0)).await;

    assert_eq!(verdict.label(), "nothing_reported");
    assert_eq!(
        signals(&heartbeat),
        ["failure: nothing reported: no continuous aggregate found"]
    );
}

/// ⚠️ The check races the stop: awaited outside the race, the stop would wait out `READ_TIMEOUT`
/// and a ping, past Docker's ten seconds.
#[tokio::test(start_paused = true)]
async fn a_stop_during_a_check_ends_the_loop_without_waiting_for_it() {
    let heartbeat = Arc::new(RecordingHeartbeat::default());
    let alarm = MaterializationAlarm::new(
        Arc::new(HangingRepository),
        Some(heartbeat.clone()),
        MaterializationAlarmSettings {
            interval: Duration::from_secs(600),
            max_wait: ChronoDuration::hours(4),
        },
    );
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(alarm.run(shutdown.clone()));

    // The first tick fires at once; one second in, the read is hanging.
    tokio::time::sleep(Duration::from_secs(1)).await;
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(5), running)
        .await
        .expect("the stop must not wait for the check")
        .expect("the alarm task must not panic");
    assert!(
        signals(&heartbeat).is_empty(),
        "a cut check signals nothing"
    );
}

/// The pending gauge of `aggregate` after one `record_pending` at 10:30.
fn pending_gauge(backlog: MaterializationBacklog) -> Option<f64> {
    let name = backlog.aggregate.clone();
    let snapshot = snapshot(|| async move { record_pending(&[backlog], at(10, 30)) });
    gauge(
        &snapshot,
        "yog_signals_materialization_pending_seconds",
        &[("aggregate", &name)],
    )
}

#[test]
fn a_waiting_row_sets_the_gauge_to_its_wait() {
    let gauge = pending_gauge(aggregate("swaps_hourly", Some(at(9, 0))));

    assert_eq!(gauge, Some(5400.0));
}

#[test]
fn an_aggregate_with_nothing_waiting_brings_the_gauge_back_to_zero() {
    let gauge = pending_gauge(aggregate("claims_hourly", None));

    // `Some(0)`, not `None`: an unset gauge would keep reading late after the aggregate caught up.
    assert_eq!(gauge, Some(0.0));
}
