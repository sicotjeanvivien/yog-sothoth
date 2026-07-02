//! Unit tests for the poller's tick logic (`poll_once`). DB-free: the
//! mock feed lens stands in for the repository; a broadcast subscriber
//! stands in for an SSE connection.

use tokio::sync::broadcast;
use yog_core::RepositoryError;
use yog_core::domain::{SignalCursor, SignalRecord};

use super::poll_once;
use crate::testing::{MockSignalRepo, make_signal_record, pk, ts};

fn cursor_of(record: &SignalRecord) -> SignalCursor {
    SignalCursor {
        triggered_at: record.signal.triggered_at,
        id: record.id,
    }
}

#[tokio::test]
async fn anchors_at_the_tip_and_emits_nothing() {
    // Fresh watermark on a non-empty feed: anchor at the tip; the delta
    // past the tip is empty — no replay of history.
    let tip = SignalCursor {
        triggered_at: ts(1_700),
        id: 9,
    };
    let repo = MockSignalRepo::feed(Ok(Some(tip.clone())), Ok(vec![]));
    let (tx, mut rx) = broadcast::channel(8);

    let watermark = poll_once(&repo, &tx, None).await;

    assert_eq!(watermark, Some(tip));
    assert!(rx.try_recv().is_err(), "nothing must be broadcast");
}

#[tokio::test]
async fn empty_feed_anchors_at_the_origin() {
    let repo = MockSignalRepo::feed(Ok(None), Ok(vec![]));
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&repo, &tx, None).await.unwrap();

    assert_eq!(watermark.id, 0);
}

#[tokio::test]
async fn broadcasts_the_delta_and_advances_the_watermark() {
    let older = make_signal_record(10, pk(1));
    let newer = make_signal_record(11, pk(2));
    let repo = MockSignalRepo::feed(
        Err(RepositoryError::Integrity("must not anchor".into())),
        Ok(vec![older.clone(), newer.clone()]),
    );
    let (tx, mut rx) = broadcast::channel(8);
    let anchor = SignalCursor {
        triggered_at: ts(1_600),
        id: 9,
    };

    let watermark = poll_once(&repo, &tx, Some(anchor)).await;

    assert_eq!(watermark, Some(cursor_of(&newer)));
    assert_eq!(rx.try_recv().unwrap().id, 10);
    assert_eq!(rx.try_recv().unwrap().id, 11);
}

#[tokio::test]
async fn anchoring_failure_retries_next_tick() {
    let repo = MockSignalRepo::feed(
        Err(RepositoryError::Integrity("db down".into())),
        Ok(vec![]),
    );
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&repo, &tx, None).await;

    assert_eq!(watermark, None, "None = re-anchor on the next tick");
}

#[tokio::test]
async fn read_failure_keeps_the_watermark() {
    let anchor = SignalCursor {
        triggered_at: ts(1_600),
        id: 9,
    };
    let repo = MockSignalRepo::feed(
        Ok(None),
        Err(RepositoryError::Integrity("db hiccup".into())),
    );
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&repo, &tx, Some(anchor.clone())).await;

    assert_eq!(
        watermark,
        Some(anchor),
        "a failed read must not move the watermark"
    );
}

#[tokio::test]
async fn no_receiver_still_advances_the_watermark() {
    // Receivers vanished between the run-loop's count check and the
    // send: the send errors, but the tick's outcome is unchanged.
    let record = make_signal_record(10, pk(1));
    let expected = cursor_of(&record);
    let repo = MockSignalRepo::feed(Ok(None), Ok(vec![record]));
    let (tx, _) = broadcast::channel::<SignalRecord>(8);
    let anchor = SignalCursor {
        triggered_at: ts(1_600),
        id: 9,
    };

    let watermark = poll_once(&repo, &tx, Some(anchor)).await;

    assert_eq!(watermark, Some(expected));
}
