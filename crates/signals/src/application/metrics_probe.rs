//! Shared test harness for the metrics a detector or a worker emits.
//! ⚠️ `with_local_recorder` binds the current thread, so the future runs inside it on a
//! current-thread runtime; `Snapshotter::snapshot` swaps counters to 0, so take one and query it.

use std::future::Future;

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

pub(crate) type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Drive `f` once under a thread-local recorder and return its snapshot.
pub(crate) fn snapshot<F, Fut>(f: F) -> Snapshot
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()>,
{
    let recorder = DebuggingRecorder::new();
    let snapshotter: Snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("current-thread runtime")
            .block_on(f());
    });

    snapshotter.snapshot().into_vec()
}

/// The counter value for `name` carrying every label in `labels`; `None` if never touched, the
/// assertion for a guard that must not count.
pub(crate) fn counter(snapshot: &Snapshot, name: &str, labels: &[(&str, &str)]) -> Option<u64> {
    match find(snapshot, name, labels)? {
        DebugValue::Counter(n) => Some(*n),
        _ => None,
    }
}

/// The gauge value for `name` carrying every label in `labels`; `None` if never set, which is not
/// `0`, the value that brings a gauge back down.
pub(crate) fn gauge(snapshot: &Snapshot, name: &str, labels: &[(&str, &str)]) -> Option<f64> {
    match find(snapshot, name, labels)? {
        DebugValue::Gauge(n) => Some(n.into_inner()),
        _ => None,
    }
}

/// The value of the metric `name` carrying every label in `labels`.
fn find<'a>(snapshot: &'a Snapshot, name: &str, labels: &[(&str, &str)]) -> Option<&'a DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| {
            key.key().name() == name
                && labels.iter().all(|(lk, lv)| {
                    key.key()
                        .labels()
                        .any(|l| l.key() == *lk && l.value() == *lv)
                })
        })
        .map(|(_, _, _, value)| value)
}
