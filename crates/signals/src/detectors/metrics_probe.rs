//! Shared test harness for asserting on the counters a detector emits.
//!
//! Lives here rather than being copy-pasted per detector test: the snapshot
//! recipe carries two traps that are silent when got wrong, and one copy of
//! each explanation is one copy to keep true.
//!
//!   * `with_local_recorder` installs the recorder on the **current thread**
//!     for the duration of a closure, so the future has to be driven *inside*
//!     it — hence the current-thread runtime rather than `#[tokio::test]`;
//!   * `Snapshotter::snapshot` is **destructive** for counters (`swap(0)`), so
//!     a second call returns zeros and would "prove" a metric that never fired.
//!     Take one snapshot and query it repeatedly.
//!
//! Same recipe as `yog-indexer`'s persistor test and `yog-context`'s
//! price-worker test.

use std::future::Future;

use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshotter};

pub(super) type Snapshot = Vec<(
    metrics_util::CompositeKey,
    Option<metrics::Unit>,
    Option<metrics::SharedString>,
    DebugValue,
)>;

/// Drive `f` once under a thread-local recorder and return its snapshot.
pub(super) fn snapshot<F, Fut>(f: F) -> Snapshot
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

/// The counter value for `name` carrying every label in `labels`.
///
/// `None` means the counter was never touched — which is the assertion that
/// matters for a guard that must NOT count (a pool below a materiality floor
/// was seen, not missed).
pub(super) fn counter(snapshot: &Snapshot, name: &str, labels: &[(&str, &str)]) -> Option<u64> {
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
        .and_then(|(_, _, _, v)| match v {
            DebugValue::Counter(n) => Some(*n),
            _ => None,
        })
}
