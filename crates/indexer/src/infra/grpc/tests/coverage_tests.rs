//! A session's range, and the writer that records it a period late.

use super::*;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};

use crate::infra::grpc::test_fixtures::{PROTOCOL, RecordedCoverage, Snapshot, at};

const PERIOD: Duration = Duration::from_secs(60);

/// Past every slot these tests close: nothing waits for a reconnection.
const FAR: Option<u64> = Some(1_000);

fn session_range(from: u64, to: u64, ended: bool) -> SessionRange {
    let mut range = SlotRange::starting(from, at(from as i64));
    range.include(to, at(to as i64));
    SessionRange {
        protocols: Arc::from([PROTOCOL]),
        range,
        ended,
    }
}

fn spans(observed: &Observed) -> Vec<(u64, u64)> {
    observed
        .snapshot()
        .values()
        .map(|entry| (entry.range.from(), entry.range.to()))
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
        (key.key().name() == "yog_indexer_grpc_coverage_writes_total"
            && key
                .key()
                .labels()
                .any(|l| l.key() == "outcome" && l.value() == outcome))
        .then_some(value)
    })
}

fn recorded(repository: &RecordedCoverage) -> Vec<(u64, u64)> {
    repository
        .rows()
        .iter()
        .map(|row| (row.range.from(), row.range.to()))
        .collect()
}

/// A writer over `observed`, run by the caller until it cancels the returned token.
fn writer(
    repository: &Arc<RecordedCoverage>,
    observed: &Observed,
) -> (
    impl std::future::Future<Output = ()> + use<>,
    CancellationToken,
) {
    let stop = CancellationToken::new();
    let run = CoverageWriter::new(repository.clone(), PERIOD).run(observed.clone(), stop.clone());
    (run, stop)
}

// ── the session's side ──────────────────────────────────────────────

/// Mutation: the high end no longer widened by a session's block-metas.
#[test]
fn each_settled_block_meta_widens_the_session_range_skipped_slots_included() {
    let observed = Observed::default();
    let mut coverage = SessionCoverage::new(Arc::from([PROTOCOL]), observed.clone());

    for slot in [10, 11, 13, 14] {
        coverage.on_block_meta(slot, at(slot as i64), FAR);
    }

    assert_eq!(spans(&observed), [(10, 14)]);
}

/// Mutation: the resume point ignored, so a slot a refused replay would lose counts as observed.
#[test]
fn a_slot_at_or_past_the_resume_point_waits_until_the_point_passes_it() {
    let observed = Observed::default();
    let mut coverage = SessionCoverage::new(Arc::from([PROTOCOL]), observed.clone());

    for slot in 10..=14 {
        coverage.on_block_meta(slot, at(slot as i64), Some(13));
    }
    assert_eq!(
        spans(&observed),
        [(10, 12)],
        "13 and 14 would be asked again"
    );

    coverage.on_block_meta(15, at(15), Some(16));
    assert_eq!(spans(&observed), [(10, 15)]);
}

/// Mutation: the empty-protocols guard removed, so a pool watched alone claims its protocol.
#[test]
fn a_session_watching_no_whole_protocol_publishes_nothing() {
    let observed = Observed::default();
    let mut coverage = SessionCoverage::new(Arc::from([]), observed.clone());

    coverage.on_block_meta(10, at(10), FAR);

    assert_eq!(spans(&observed), []);
}

/// Mutation: a session dropped without its entry marked ended, so it is never forgotten.
#[test]
fn a_dropped_session_marks_its_range_ended() {
    let observed = Observed::default();
    let mut coverage = SessionCoverage::new(Arc::from([PROTOCOL]), observed.clone());
    coverage.on_block_meta(10, at(10), FAR);
    drop(coverage);

    let entries = observed.snapshot();
    assert!(entries.values().all(|entry| entry.ended), "{entries:?}");
}

// ── the writer ──────────────────────────────────────────────────────

/// Mutation: the writer records what it reads at a tick, not what it read at the tick before.
#[test]
fn each_tick_writes_the_range_read_at_the_tick_before() {
    let repository = Arc::new(RecordedCoverage::default());
    let observed = Observed::default();
    observed.publish(at(0), session_range(10, 20, false));
    let (run, stop) = writer(&repository, &observed);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD / 2).await;
        assert_eq!(recorded(&repository), [], "the first tick only reads");

        observed.publish(at(0), session_range(10, 30, false));
        tokio::time::sleep(PERIOD).await;
        assert_eq!(
            recorded(&repository),
            [(10, 20)],
            "a tick writes what it read the tick before, not [10, 30]"
        );

        tokio::time::sleep(PERIOD).await;
        assert_eq!(recorded(&repository), [(10, 20), (10, 30)]);

        stop.cancel();
        task.await.expect("the writer ends on stop");
    });
}

/// Mutation: one entry for every session, so a reconnection overwrites the last range before.
#[test]
fn a_session_ending_between_two_ticks_still_has_its_last_range_written() {
    let repository = Arc::new(RecordedCoverage::default());
    let observed = Observed::default();
    observed.publish(at(0), session_range(10, 20, false));
    let (run, stop) = writer(&repository, &observed);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD / 2).await;
        // Between two ticks: the first session ends wider, and a second one starts.
        observed.publish(at(0), session_range(10, 25, true));
        observed.publish(at(1), session_range(26, 30, false));
        tokio::time::sleep(PERIOD * 2).await;

        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    assert!(
        recorded(&repository).contains(&(10, 25)),
        "{:?}",
        recorded(&repository)
    );
    assert!(
        recorded(&repository).contains(&(26, 30)),
        "{:?}",
        recorded(&repository)
    );
}

/// Mutation: an unchanged range written again at every tick.
#[test]
fn an_unchanged_range_is_written_once() {
    let repository = Arc::new(RecordedCoverage::default());
    let observed = Observed::default();
    observed.publish(at(0), session_range(10, 20, false));
    let (run, stop) = writer(&repository, &observed);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD * 4).await;
        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    assert_eq!(recorded(&repository), [(10, 20)]);
}

/// Mutation: an ended range kept once recorded, so a run's entries grow for ever.
#[test]
fn an_ended_range_is_forgotten_once_recorded() {
    let repository = Arc::new(RecordedCoverage::default());
    let observed = Observed::default();
    observed.publish(at(0), session_range(10, 20, true));
    let (run, stop) = writer(&repository, &observed);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    assert_eq!(recorded(&repository), [(10, 20)]);
    assert_eq!(spans(&observed), []);
}

/// Mutation: the writer returning at `stop` without writing what is due.
#[test]
fn a_stop_writes_what_was_due() {
    let repository = Arc::new(RecordedCoverage::default());
    let observed = Observed::default();
    observed.publish(at(0), session_range(10, 20, false));
    let (run, stop) = writer(&repository, &observed);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD / 2).await;
        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    assert_eq!(recorded(&repository), [(10, 20)]);
}

/// Mutation: a failed write ending the writer, dropping its range, or not counted.
#[test]
fn a_refused_write_is_counted_and_the_next_tick_writes_again() {
    let repository = Arc::new(RecordedCoverage::default());
    repository.refuse(true);
    let observed = Observed::default();
    observed.publish(at(0), session_range(10, 20, true));
    let (run, stop) = writer(&repository, &observed);

    let ((), snapshot) = paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        assert_eq!(recorded(&repository), [], "the first write was refused");

        repository.refuse(false);
        tokio::time::sleep(PERIOD).await;
        assert_eq!(recorded(&repository), [(10, 20)], "the writer went on");

        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    assert_eq!(writes(&snapshot, "failed"), Some(&DebugValue::Counter(1)));
    assert_eq!(writes(&snapshot, "written"), Some(&DebugValue::Counter(1)));
}

/// Mutation: one write per range instead of one per protocol.
#[test]
fn a_range_is_written_once_per_protocol_watched_whole() {
    let repository = Arc::new(RecordedCoverage::default());
    let observed = Observed::default();
    let mut entry = session_range(10, 20, false);
    entry.protocols = Arc::from([PROTOCOL, Protocol::MeteoraDlmm]);
    observed.publish(at(0), entry);
    let (run, stop) = writer(&repository, &observed);

    paused(async {
        let task = tokio::spawn(run);
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    let protocols: Vec<Protocol> = repository.rows().iter().map(|row| row.protocol).collect();
    assert_eq!(protocols, [PROTOCOL, Protocol::MeteoraDlmm]);
}
