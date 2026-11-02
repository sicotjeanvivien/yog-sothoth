//! A session's range, and the writer that records it a period late.

use super::*;

use metrics_util::debugging::{DebugValue, DebuggingRecorder};

use crate::infra::grpc::test_fixtures::{PROTOCOL, RecordedCoverage, Snapshot, at};

const PERIOD: Duration = Duration::from_secs(60);

fn published(started: i64, from: u64, to: u64) -> SessionRange {
    let mut range = SlotRange::starting(from, at(from as i64));
    range.include(to, at(to as i64));
    SessionRange {
        started_at: at(started),
        protocols: Arc::from([PROTOCOL]),
        range,
    }
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

fn ranges(repository: &RecordedCoverage) -> Vec<(u64, u64)> {
    repository
        .rows()
        .iter()
        .map(|row| (row.range.from(), row.range.to()))
        .collect()
}

// ── the session's side ──────────────────────────────────────────────

/// Mutation: the high end no longer widened by a session's block-metas.
#[test]
fn each_timed_block_meta_widens_what_the_session_publishes() {
    let (publish, published) = watch::channel(None);
    let mut coverage = SessionCoverage::new(Arc::from([PROTOCOL]), publish);

    for slot in [10, 11, 13, 14] {
        coverage.on_block_meta(slot, at(slot as i64));
    }

    let seen = published.borrow().clone().expect("a range is published");
    assert_eq!(&*seen.protocols, [PROTOCOL]);
    assert_eq!((seen.range.from(), seen.range.to()), (10, 14));
    assert_eq!(seen.range.to_at(), at(14));
}

/// Mutation: the empty-protocols guard removed, so a pool watched alone claims its protocol.
#[test]
fn a_session_watching_no_whole_protocol_publishes_nothing() {
    let (publish, published) = watch::channel(None);
    let mut coverage = SessionCoverage::new(Arc::from([]), publish);

    coverage.on_block_meta(10, at(10));

    assert_eq!(*published.borrow(), None);
}

// ── the writer ──────────────────────────────────────────────────────

/// Mutation: the writer records what it reads at a tick, not what it read at the tick before.
#[test]
fn each_tick_writes_the_range_read_at_the_tick_before() {
    let repository = Arc::new(RecordedCoverage::default());
    let writer = CoverageWriter::new(repository.clone(), PERIOD);
    let (publish, rx) = watch::channel(Some(published(0, 10, 20)));
    let stop = CancellationToken::new();

    paused(async {
        let task = tokio::spawn(writer.run(rx, stop.clone()));
        tokio::time::sleep(PERIOD / 2).await;
        assert_eq!(ranges(&repository), [], "the first tick only reads");

        publish.send_replace(Some(published(0, 10, 30)));
        tokio::time::sleep(PERIOD).await;
        assert_eq!(
            ranges(&repository),
            [(10, 20)],
            "a tick writes what it read the tick before, not [10, 30]"
        );

        tokio::time::sleep(PERIOD).await;
        assert_eq!(ranges(&repository), [(10, 20), (10, 30)]);

        stop.cancel();
        task.await.expect("the writer ends on stop");
    });
}

/// Mutation: a failed write ending the writer, or not counted.
#[test]
fn a_refused_write_is_counted_and_the_next_tick_writes_again() {
    let repository = Arc::new(RecordedCoverage::default());
    repository.refuse(true);
    let writer = CoverageWriter::new(repository.clone(), PERIOD);
    let (_publish, rx) = watch::channel(Some(published(0, 10, 20)));
    let stop = CancellationToken::new();

    let ((), snapshot) = paused(async {
        let task = tokio::spawn(writer.run(rx, stop.clone()));
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        assert_eq!(ranges(&repository), [], "the first write was refused");

        repository.refuse(false);
        tokio::time::sleep(PERIOD).await;
        assert_eq!(ranges(&repository), [(10, 20)], "the writer went on");

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
    let writer = CoverageWriter::new(repository.clone(), PERIOD);
    let mut range = published(0, 10, 20);
    range.protocols = Arc::from([PROTOCOL, Protocol::MeteoraDlmm]);
    let (_publish, rx) = watch::channel(Some(range));
    let stop = CancellationToken::new();

    paused(async {
        let task = tokio::spawn(writer.run(rx, stop.clone()));
        tokio::time::sleep(PERIOD + PERIOD / 2).await;
        stop.cancel();
        task.await.expect("the writer ends on stop");
    });

    let protocols: Vec<Protocol> = repository.rows().iter().map(|row| row.protocol).collect();
    assert_eq!(protocols, [PROTOCOL, Protocol::MeteoraDlmm]);
}
