//! DB-free tests of the live poller: a tick on a scripted source, and the loop's stops.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use yog_core::{RepositoryError, RepositoryResult};

use super::{LivePoller, poll_once};
use crate::application::live::LiveSource;
use crate::application::live::watermark::LiveWatermark;

/// A source whose records are their ids, answering from a script; an exhausted script fails.
#[derive(Default)]
struct ScriptedSource {
    tips: Mutex<VecDeque<RepositoryResult<Option<i64>>>>,
    reads: Mutex<VecDeque<RepositoryResult<Vec<i64>>>>,
    /// The `after_id` of each read.
    asked: Mutex<Vec<i64>>,
    calls: AtomicUsize,
}

impl ScriptedSource {
    fn new(
        tips: Vec<RepositoryResult<Option<i64>>>,
        reads: Vec<RepositoryResult<Vec<i64>>>,
    ) -> Self {
        Self {
            tips: Mutex::new(tips.into()),
            reads: Mutex::new(reads.into()),
            ..Self::default()
        }
    }
}

fn exhausted() -> RepositoryError {
    RepositoryError::Integrity("script exhausted".into())
}

impl LiveSource for ScriptedSource {
    type Record = i64;
    type Item = i64;

    const NAME: &'static str = "scripted";

    async fn tip(&self) -> RepositoryResult<Option<i64>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.tips
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(exhausted()))
    }

    async fn after(&self, after_id: i64, _limit: i64) -> RepositoryResult<Vec<i64>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push(after_id);
        self.reads
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Err(exhausted()))
    }

    fn id_of(record: &i64) -> i64 {
        *record
    }

    async fn prepare(&self, records: Vec<i64>) -> Vec<i64> {
        records
    }
}

fn received(rx: &mut broadcast::Receiver<Arc<i64>>) -> Vec<i64> {
    std::iter::from_fn(|| rx.try_recv().ok().map(|id| *id)).collect()
}

#[tokio::test]
async fn a_fresh_watermark_anchors_at_the_tip_and_emits_nothing() {
    let source = ScriptedSource::new(vec![Ok(Some(9))], vec![Ok(vec![])]);
    let (tx, mut rx) = broadcast::channel(8);

    let watermark = poll_once(&source, &tx, None).await.expect("anchored");

    assert_eq!(watermark.floor(), 9);
    assert_eq!(*source.asked.lock().unwrap(), vec![9]);
    assert!(received(&mut rx).is_empty());
}

#[tokio::test]
async fn an_empty_table_anchors_at_the_origin() {
    let source = ScriptedSource::new(vec![Ok(None)], vec![Ok(vec![])]);
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&source, &tx, None).await.expect("anchored");

    assert_eq!(watermark.floor(), 0);
}

#[tokio::test]
async fn an_anchoring_failure_retries_next_tick() {
    let source = ScriptedSource::new(vec![Err(exhausted())], vec![]);
    let (tx, _rx) = broadcast::channel(8);

    assert!(poll_once(&source, &tx, None).await.is_none());
}

#[tokio::test]
async fn a_read_failure_keeps_the_watermark() {
    let source = ScriptedSource::new(vec![], vec![Err(exhausted())]);
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&source, &tx, Some(LiveWatermark::anchored(9)))
        .await
        .expect("kept");

    assert_eq!(watermark.floor(), 9);
}

/// Mutation: the delivered ids forgotten — the 11 read again behind the hole at 10 goes out twice.
#[tokio::test]
async fn a_record_read_twice_is_broadcast_once() {
    let source = ScriptedSource::new(vec![], vec![Ok(vec![11]), Ok(vec![10, 11])]);
    let (tx, mut rx) = broadcast::channel(8);

    let watermark = poll_once(&source, &tx, Some(LiveWatermark::anchored(9))).await;
    let watermark = poll_once(&source, &tx, watermark).await.expect("kept");

    assert_eq!(received(&mut rx), vec![11, 10]);
    assert_eq!(watermark.floor(), 11);
    assert_eq!(*source.asked.lock().unwrap(), vec![9, 9]);
}

/// Receivers left between the count check and the send: the tick is unchanged.
#[tokio::test]
async fn no_receiver_still_advances_the_watermark() {
    let source = ScriptedSource::new(vec![], vec![Ok(vec![10])]);
    let (tx, _) = broadcast::channel::<Arc<i64>>(8);

    let watermark = poll_once(&source, &tx, Some(LiveWatermark::anchored(9)))
        .await
        .expect("kept");

    assert_eq!(watermark.floor(), 10);
}

/// Mutation: the count of receivers no longer checked — an idle poller reads its table.
#[tokio::test]
async fn nobody_listening_means_no_read() {
    let source = Arc::new(ScriptedSource::default());
    let (tx, rx) = broadcast::channel::<Arc<i64>>(8);
    drop(rx);
    let poller = LivePoller::new(source.clone(), tx, Duration::from_millis(5));
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(poller.run(shutdown.clone()));

    tokio::time::sleep(Duration::from_millis(60)).await;
    shutdown.cancel();
    running.await.unwrap().unwrap();

    assert_eq!(source.calls.load(Ordering::SeqCst), 0);
}

/// Mutation: remove the cancellation arm of `run`, and the loop never ends — the timeout
/// below turns that into a failure instead of a hang.
#[tokio::test]
async fn the_poller_stops_when_the_token_is_cancelled() {
    let (tx, _) = broadcast::channel(8);
    let poller = LivePoller::new(
        Arc::new(ScriptedSource::default()),
        tx,
        Duration::from_secs(3600),
    );
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(poller.run(shutdown.clone()));

    // Let the first tick go by, so the loop is parked on the next one.
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .expect("the poller did not stop")
        .unwrap()
        .unwrap();
}

/// A source that never answers: a tick stuck on a slow database.
struct StuckSource;

impl LiveSource for StuckSource {
    type Record = i64;
    type Item = i64;

    const NAME: &'static str = "stuck";

    async fn tip(&self) -> RepositoryResult<Option<i64>> {
        std::future::pending().await
    }

    async fn after(&self, _after_id: i64, _limit: i64) -> RepositoryResult<Vec<i64>> {
        std::future::pending().await
    }

    fn id_of(record: &i64) -> i64 {
        *record
    }

    async fn prepare(&self, records: Vec<i64>) -> Vec<i64> {
        records
    }
}

/// Mutation: the tick awaited outside the `select!` — a stop mid-tick hangs the poller.
#[tokio::test]
async fn a_stop_mid_tick_does_not_wait_for_the_tick() {
    let (tx, _subscriber) = broadcast::channel(8);
    let poller = LivePoller::new(Arc::new(StuckSource), tx, Duration::from_millis(10));
    let shutdown = CancellationToken::new();
    let running = tokio::spawn(poller.run(shutdown.clone()));

    // Let a tick start and get stuck on the source.
    tokio::time::sleep(Duration::from_millis(100)).await;
    shutdown.cancel();

    tokio::time::timeout(Duration::from_secs(1), running)
        .await
        .expect("the poller waited for its stuck tick")
        .unwrap()
        .unwrap();
}
