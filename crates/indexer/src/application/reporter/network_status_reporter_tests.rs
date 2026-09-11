use super::*;

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};
use serde_json::{Value, json};
use solana_rpc_client::mock_sender::MocksMap;
use solana_rpc_client_api::request::RpcRequest;
use yog_core::{RepositoryError, RepositoryResult};

/// A repository that records what it was given, and fails when told to.
///
/// Each `upsert` pops the next scripted outcome; once the script is empty it
/// succeeds. A failed write records nothing, as a real one would not.
#[derive(Default)]
struct ScriptedRepository {
    outcomes: Mutex<VecDeque<RepositoryResult<()>>>,
    written: Mutex<Vec<NetworkStatus>>,
}

impl ScriptedRepository {
    fn failing_once() -> Self {
        Self {
            outcomes: Mutex::new(VecDeque::from([Err(RepositoryError::Backend(
                "connection pool timed out".to_string(),
            ))])),
            written: Mutex::default(),
        }
    }

    fn slots(&self) -> Vec<u64> {
        self.written
            .lock()
            .expect("lock")
            .iter()
            .map(|s| s.slot)
            .collect()
    }
}

#[async_trait]
impl NetworkStatusRepository for ScriptedRepository {
    async fn upsert(&self, status: &NetworkStatus) -> RepositoryResult<()> {
        let outcome = self
            .outcomes
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or(Ok(()));
        if outcome.is_ok() {
            self.written.lock().expect("lock").push(status.clone());
        }
        outcome
    }
}

/// A reporter whose `getSlot` answers are the given values, in order.
///
/// A `Value::Null` is an RPC answer that cannot be read as a slot, which is
/// how the mock client produces a failed call.
fn reporter(get_slot: &[Value], repository: Arc<ScriptedRepository>) -> NetworkStatusReporter {
    let mocks: MocksMap = get_slot
        .iter()
        .map(|value| (RpcRequest::GetSlot, value.clone()))
        .collect();
    NetworkStatusReporter::new(
        Arc::new(RpcClient::new_mock_with_mocks_map("succeeds", mocks)),
        SecretUrl::for_tests("succeeds"),
        repository,
    )
}

/// Drive `body` on a current-thread runtime with a local recorder installed.
///
/// Not `#[tokio::test]`: `with_local_recorder` installs the recorder on the
/// *current thread* for the duration of a closure, so the future is driven
/// inside it — the recipe `infra/grpc/session_tests.rs` uses.
fn with_recorder(body: impl Future<Output = ()>) -> Snapshotter {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime")
            .block_on(body);
    });
    snapshotter
}

/// The tick-failure counter for one `reason` label, or `None` when it was never
/// touched.
fn failures_for(snapshotter: &Snapshotter, reason: &str) -> Option<DebugValue> {
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_network_status_tick_failures_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "reason" && l.value() == reason)
        })
        .map(|(_, _, _, value)| value)
}

/// ⚠️ **The defect of 9 September 2026, one tick at a time.** A failed
/// `getSlot` used to leave `run` with `?` and take the daemon down; the network
/// came back, and the next tick never happened. Here the failed tick writes
/// nothing, is counted as an RPC problem — and the next one writes.
#[test]
fn a_failed_get_slot_is_counted_and_the_next_tick_records() {
    let repository = Arc::new(ScriptedRepository::default());
    let reporter = reporter(&[Value::Null, json!(42)], Arc::clone(&repository));

    let snapshotter = with_recorder(async {
        reporter.tick().await;
        assert_eq!(
            repository.slots(),
            Vec::<u64>::new(),
            "a slot that could not be read must not be written as some default"
        );

        reporter.tick().await;
    });

    assert_eq!(
        repository.slots(),
        vec![42],
        "the tick after a failure is the one that records"
    );
    assert_eq!(
        failures_for(&snapshotter, "rpc"),
        Some(DebugValue::Counter(1))
    );
    assert_eq!(
        failures_for(&snapshotter, "persistence"),
        None,
        "an unreachable RPC is not a database problem"
    );
}

/// The same policy for the other half of a tick: a write that fails is
/// counted under its own reason, and does not stop the next one.
///
/// ⚠️ This is the failure `index_concurrency` reserves a connection against —
/// an `acquire_timeout` on a saturated pool — and until 11 September 2026 it
/// stopped the process too.
#[test]
fn a_failed_write_is_counted_and_the_next_tick_records() {
    let repository = Arc::new(ScriptedRepository::failing_once());
    let reporter = reporter(&[json!(41), json!(42)], Arc::clone(&repository));

    let snapshotter = with_recorder(async {
        reporter.tick().await;
        reporter.tick().await;
    });

    assert_eq!(repository.slots(), vec![42]);
    assert_eq!(
        failures_for(&snapshotter, "persistence"),
        Some(DebugValue::Counter(1))
    );
    assert_eq!(
        failures_for(&snapshotter, "rpc"),
        None,
        "a slot that was read is not an RPC problem"
    );
}

/// `run` stops on cancellation, and that is its only way out: its error type
/// is `Infallible`. The token is cancelled before the call, so no clock has to
/// be paused — the first tick and the cancellation are both ready, and either
/// order ends in `Ok`.
#[tokio::test]
async fn run_returns_when_cancelled() {
    let repository = Arc::new(ScriptedRepository::default());
    let reporter = reporter(&[json!(42)], repository);
    let shutdown = CancellationToken::new();
    shutdown.cancel();

    let result = tokio::time::timeout(Duration::from_secs(5), reporter.run(shutdown)).await;

    assert!(
        matches!(result, Ok(Ok(()))),
        "a cancelled reporter returns promptly"
    );
}

/// ⚠️ **The loop, not one tick.** The two tests above call `tick` by hand, so a
/// `run` that returned `Ok` after its first failure would pass them — and
/// `Daemon::run` cancels everything when the reporter's task ends, `Ok` or not:
/// the defect of 9 September with a clean exit code instead of a loud one.
/// `Infallible` forbids the `?`, not the early return; this test forbids that.
#[tokio::test(start_paused = true)]
async fn run_keeps_ticking_after_a_failed_tick() {
    let repository = Arc::new(ScriptedRepository::default());
    let reporter = reporter(&[Value::Null, json!(42)], Arc::clone(&repository));
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(reporter.run(shutdown.clone()));

    // The first tick fires at once and fails; the second is due one interval
    // later. The paused clock jumps there as soon as the runtime is idle.
    tokio::time::sleep(TICK_INTERVAL + Duration::from_secs(1)).await;

    assert_eq!(
        repository.slots(),
        vec![42],
        "the tick after the failed one ran, and recorded"
    );
    assert!(
        !task.is_finished(),
        "a failed tick must not end the reporter's task"
    );

    shutdown.cancel();
    assert!(matches!(task.await, Ok(Ok(()))));
}

/// A repository whose first write takes `first_write`, counting every call.
struct SlowFirstWrite {
    first_write: Duration,
    calls: Mutex<usize>,
}

#[async_trait]
impl NetworkStatusRepository for SlowFirstWrite {
    async fn upsert(&self, _status: &NetworkStatus) -> RepositoryResult<()> {
        let first = {
            let mut calls = self.calls.lock().expect("lock");
            *calls += 1;
            *calls == 1
        };
        if first {
            tokio::time::sleep(self.first_write).await;
        }
        Ok(())
    }
}

/// ⚠️ **Ticks missed during a slow one are not replayed in a burst.** Seen on
/// 11 September 2026 under tokio's default: three failed ticks logged in the
/// same millisecond when a two-minute cut ended.
///
/// A first tick that takes 40 s misses the ticks due at 15 s and 30 s. `Burst`
/// fires both the moment it returns — three calls by 41 s; `Delay` fires one and
/// waits a full interval before the next — two.
#[tokio::test(start_paused = true)]
async fn ticks_missed_during_a_slow_tick_are_not_replayed_in_a_burst() {
    let repository = Arc::new(SlowFirstWrite {
        first_write: Duration::from_secs(40),
        calls: Mutex::new(0),
    });
    let reporter = NetworkStatusReporter::new(
        Arc::new(RpcClient::new_mock_with_mocks_map(
            "succeeds",
            MocksMap::default(),
        )),
        SecretUrl::for_tests("succeeds"),
        Arc::clone(&repository) as Arc<dyn NetworkStatusRepository>,
    );
    let shutdown = CancellationToken::new();
    let task = tokio::spawn(reporter.run(shutdown.clone()));

    tokio::time::sleep(Duration::from_secs(41)).await;

    assert_eq!(
        *repository.calls.lock().expect("lock"),
        2,
        "the slow tick, then one catch-up tick — not one per missed interval"
    );

    shutdown.cancel();
    assert!(matches!(task.await, Ok(Ok(()))));
}
