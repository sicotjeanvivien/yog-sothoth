//! The recorder: what it writes, when, and what it does with a refusal.

use super::*;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};

use crate::infra::grpc::test_fixtures::{PROTOCOL, Snapshot, WrittenSlots, at};

const PERIOD: Duration = Duration::from_secs(60);

fn range(first: u64, last: u64, ended: bool) -> RangeToRecord {
    let mut span = ReceivedSpan::starting(first, at(first as i64));
    span.include(last, at(last as i64));
    RangeToRecord {
        protocols: Arc::from([PROTOCOL]),
        span,
        ended,
    }
}

fn spans(ranges: &RangesToRecord) -> Vec<(u64, u64)> {
    ranges
        .snapshot()
        .values()
        .map(|range| (range.span.first().0, range.span.last().0))
        .collect()
}

/// Runs `future` on a paused current-thread runtime, under a local metrics recorder.
/// ⚠️ `with_local_recorder` binds the current thread, hence `#[test]` and not `#[tokio::test]`.
fn paused<T>(future: impl std::future::Future<Output = T>) -> (T, Snapshot) {
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let output = metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .expect("current-thread runtime")
            .block_on(future)
    });
    (output, snapshotter.snapshot().into_vec())
}

fn writes<'a>(snapshot: &'a Snapshot, outcome: &str) -> Option<&'a DebugValue> {
    snapshot.iter().find_map(|(key, _, _, value)| {
        (key.key().name() == "yog_indexer_grpc_observed_slots_writes_total"
            && key
                .key()
                .labels()
                .any(|l| l.key() == "outcome" && l.value() == outcome))
        .then_some(value)
    })
}

fn written(repository: &WrittenSlots) -> Vec<(u64, u64)> {
    repository
        .rows()
        .iter()
        .map(|row| (row.first_slot, row.last_slot))
        .collect()
}

/// A recorder over `ranges`, run by the caller until it cancels the returned token.
fn recorder(
    repository: &Arc<WrittenSlots>,
    ranges: &RangesToRecord,
) -> (
    impl std::future::Future<Output = ()> + use<>,
    CancellationToken,
) {
    let stop = CancellationToken::new();
    let run = SlotsRecorder::new(repository.clone(), PERIOD).run(ranges.clone(), stop.clone());
    (run, stop)
}

/// Mutation: the recorder writes what it reads at a tick, not what it read at the tick before.
#[test]
fn each_tick_writes_the_span_read_at_the_tick_before() {
    let repository = Arc::new(WrittenSlots::default());
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20, false));
    let (run, stop) = recorder(&repository, &ranges);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD / 2).await;
        assert_eq!(written(&repository), [], "the first tick only reads");

        ranges.publish(at(0), range(10, 30, false));
        tokio::time::sleep(PERIOD).await;
        assert_eq!(
            written(&repository),
            [(10, 20)],
            "a tick writes what it read the tick before, not [10, 30]"
        );

        tokio::time::sleep(PERIOD).await;
        assert_eq!(written(&repository), [(10, 20), (10, 30)]);

        stop.cancel();
        task.await.expect("the recorder ends on stop");
    });
}

/// Mutation: one entry for every session, so a reconnection overwrites the last span before.
#[test]
fn a_session_ending_between_two_ticks_still_has_its_last_span_written() {
    let repository = Arc::new(WrittenSlots::default());
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20, false));
    let (run, stop) = recorder(&repository, &ranges);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD / 2).await;
        // Between two ticks: the first session ends wider, and a second one starts.
        ranges.publish(at(0), range(10, 25, true));
        ranges.publish(at(1), range(26, 30, false));
        tokio::time::sleep(PERIOD * 2).await;

        stop.cancel();
        task.await.expect("the recorder ends on stop");
    });

    let written = written(&repository);
    assert!(written.contains(&(10, 25)), "{written:?}");
    assert!(written.contains(&(26, 30)), "{written:?}");
}

/// Mutation: an unchanged span written again at every tick.
#[test]
fn an_unchanged_span_is_written_once() {
    let repository = Arc::new(WrittenSlots::default());
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20, false));
    let (run, stop) = recorder(&repository, &ranges);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD * 4).await;
        stop.cancel();
        task.await.expect("the recorder ends on stop");
    });

    assert_eq!(written(&repository), [(10, 20)]);
}

/// Mutation: an ended span kept once written, so a run's entries grow for ever.
#[test]
fn an_ended_span_is_forgotten_once_written() {
    let repository = Arc::new(WrittenSlots::default());
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20, true));
    let (run, stop) = recorder(&repository, &ranges);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        stop.cancel();
        task.await.expect("the recorder ends on stop");
    });

    assert_eq!(written(&repository), [(10, 20)]);
    assert_eq!(spans(&ranges), []);
}

/// Mutation: the recorder returning at `stop` without writing what is due.
#[test]
fn a_stop_writes_what_was_due() {
    let repository = Arc::new(WrittenSlots::default());
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20, false));
    let (run, stop) = recorder(&repository, &ranges);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD / 2).await;
        stop.cancel();
        task.await.expect("the recorder ends on stop");
    });

    assert_eq!(written(&repository), [(10, 20)]);
}

/// Mutation: a refused write ending the recorder, dropping its span, or not counted.
#[test]
fn a_refused_write_is_counted_and_the_next_tick_writes_again() {
    let repository = Arc::new(WrittenSlots::default());
    repository.refuse(true);
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20, true));
    let (run, stop) = recorder(&repository, &ranges);

    let ((), snapshot) = paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        assert_eq!(written(&repository), [], "the first write was refused");

        repository.refuse(false);
        tokio::time::sleep(PERIOD).await;
        assert_eq!(written(&repository), [(10, 20)], "the recorder went on");

        stop.cancel();
        task.await.expect("the recorder ends on stop");
    });

    assert_eq!(writes(&snapshot, "failed"), Some(&DebugValue::Counter(1)));
    assert_eq!(writes(&snapshot, "written"), Some(&DebugValue::Counter(1)));
}
