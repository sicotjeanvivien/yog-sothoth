//! Unit tests for the poller's tick logic (`poll_once`). DB-free: the
//! mock feed lens stands in for the repository; a broadcast subscriber
//! stands in for an SSE connection.

use std::sync::Arc;

use tokio::sync::broadcast;
use yog_core::RepositoryError;
use yog_core::domain::{Pool, SignalCursor, SignalRecord};

use super::poll_once;
use crate::application::{EnrichedSignal, SignalService};
use crate::testing::{
    MockMetadataRepo, MockPriceRepo, MockSignalRepo, PoolRepoOnce, make_metadata, make_pool,
    make_signal_record, pk, ts,
};

/// An enricher whose catalog answers **one** batch lookup — a second call
/// panics (`PoolRepoOnce`), which is what pins "one enrichment per tick".
fn enricher(pools: Vec<Pool>, metadata: MockMetadataRepo) -> SignalService {
    SignalService::new(
        Arc::new(MockSignalRepo::empty()),
        Arc::new(PoolRepoOnce::with_pools(pools)),
        Arc::new(metadata),
        Arc::new(MockPriceRepo::empty()),
    )
}

fn symbol(token: &crate::application::EnrichedToken) -> Option<&str> {
    token.metadata.as_ref().and_then(|m| m.symbol.as_deref())
}

/// For ticks whose signals need no resolved pair.
fn idle_enricher() -> SignalService {
    enricher(vec![], MockMetadataRepo::empty())
}

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

    let watermark = poll_once(&repo, &idle_enricher(), &tx, None).await;

    assert_eq!(watermark, Some(tip));
    assert!(rx.try_recv().is_err(), "nothing must be broadcast");
}

#[tokio::test]
async fn empty_feed_anchors_at_the_origin() {
    let repo = MockSignalRepo::feed(Ok(None), Ok(vec![]));
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&repo, &idle_enricher(), &tx, None).await.unwrap();

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

    let watermark = poll_once(&repo, &idle_enricher(), &tx, Some(anchor)).await;

    assert_eq!(watermark, Some(cursor_of(&newer)));
    assert_eq!(rx.try_recv().unwrap().record.id, 10);
    assert_eq!(rx.try_recv().unwrap().record.id, 11);
}

#[tokio::test]
async fn anchoring_failure_retries_next_tick() {
    let repo = MockSignalRepo::feed(
        Err(RepositoryError::Integrity("db down".into())),
        Ok(vec![]),
    );
    let (tx, _rx) = broadcast::channel(8);

    let watermark = poll_once(&repo, &idle_enricher(), &tx, None).await;

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

    let watermark = poll_once(&repo, &idle_enricher(), &tx, Some(anchor.clone())).await;

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
    let (tx, _) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    let anchor = SignalCursor {
        triggered_at: ts(1_600),
        id: 9,
    };

    let watermark = poll_once(&repo, &idle_enricher(), &tx, Some(anchor)).await;

    assert_eq!(watermark, Some(expected));
}

/// Mutation: remove the cancellation arm of `run`, and the loop never ends —
/// the timeout below turns that into a failure instead of a hang.
#[tokio::test]
async fn the_poller_stops_when_the_token_is_cancelled() {
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    use super::SignalStreamPoller;

    let (tx, _) = broadcast::channel(8);
    let poller = SignalStreamPoller::new(
        Arc::new(MockSignalRepo::feed(Ok(None), Ok(vec![]))),
        Arc::new(idle_enricher()),
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

/// The fix for the per-stream enrichment: a tick's signals are enriched in one
/// batch, **before** the broadcast, and every subscriber receives the same
/// enriched signals. Three subscribers and two signals cost one catalog
/// lookup — `PoolRepoOnce` panics on a second.
///
/// Mutation: enrich each record on its own inside the send loop, and the
/// second lookup panics.
#[tokio::test]
async fn a_tick_is_enriched_once_whatever_the_number_of_subscribers() {
    let (mint_a, mint_b) = (pk(10), pk(11));
    let pools = vec![
        make_pool(pk(1), mint_a, mint_b),
        make_pool(pk(2), mint_a, mint_b),
    ];
    let metadata = MockMetadataRepo::with(vec![
        (mint_a, make_metadata(mint_a, "SOL")),
        (mint_b, make_metadata(mint_b, "USDC")),
    ]);
    let repo = MockSignalRepo::feed(
        Ok(None),
        Ok(vec![
            make_signal_record(10, pk(1)),
            make_signal_record(11, pk(2)),
        ]),
    );
    let (tx, _) = broadcast::channel(8);
    let mut subscribers: Vec<_> = (0..3).map(|_| tx.subscribe()).collect();
    let anchor = SignalCursor {
        triggered_at: ts(1_600),
        id: 9,
    };

    poll_once(&repo, &enricher(pools, metadata), &tx, Some(anchor)).await;

    for rx in &mut subscribers {
        for expected_id in [10, 11] {
            let signal = rx.try_recv().expect("every subscriber gets every signal");
            assert_eq!(signal.record.id, expected_id);
            assert_eq!(symbol(&signal.token_a), Some("SOL"));
            assert_eq!(symbol(&signal.token_b), Some("USDC"));
        }
    }
}

/// Delivering beats decorating: when the enrichment fails, the signals still
/// go out, bare, and the watermark still advances.
#[tokio::test]
async fn a_failed_enrichment_still_delivers_the_signals_bare() {
    let record = make_signal_record(10, pk(1));
    let expected = cursor_of(&record);
    let repo = MockSignalRepo::feed(Ok(None), Ok(vec![record]));
    let (tx, mut rx) = broadcast::channel(8);
    let anchor = SignalCursor {
        triggered_at: ts(1_600),
        id: 9,
    };
    let failing = enricher(
        vec![make_pool(pk(1), pk(10), pk(11))],
        MockMetadataRepo::failing(),
    );

    let watermark = poll_once(&repo, &failing, &tx, Some(anchor)).await;

    assert_eq!(watermark, Some(expected));
    let signal = rx.try_recv().expect("the signal goes out bare");
    assert_eq!(signal.record.id, 10);
    assert_eq!(symbol(&signal.token_a), None);
}
