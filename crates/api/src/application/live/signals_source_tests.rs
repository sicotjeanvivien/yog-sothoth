use std::sync::Arc;

use tokio::sync::broadcast;
use yog_core::domain::Pool;

use super::SignalsSource;
use crate::application::live::poller::poll_once;
use crate::application::live::watermark::LiveWatermark;
use crate::application::{EnrichedSignal, SignalService};
use crate::testing::{
    MockMetadataRepo, MockSignalRepo, PoolRepoOnce, make_metadata, make_pool, make_signal_record,
    pk,
};

/// An enricher whose catalog answers **one** batch lookup — a second call panics
/// (`PoolRepoOnce`), which is what pins "one enrichment per tick".
fn enricher(pools: Vec<Pool>, metadata: MockMetadataRepo) -> Arc<SignalService> {
    Arc::new(SignalService::new(
        Arc::new(MockSignalRepo::empty()),
        Arc::new(PoolRepoOnce::with_pools(pools)),
        Arc::new(metadata),
    ))
}

fn symbol(token: &crate::application::EnrichedToken) -> Option<&str> {
    token.metadata.as_ref().and_then(|m| m.symbol.as_deref())
}

/// Mutation: each record enriched on its own inside the send loop — three subscribers and
/// two signals must cost one catalog lookup.
#[tokio::test]
async fn a_tick_of_signals_is_enriched_once_whatever_the_number_of_subscribers() {
    let (mint_a, mint_b) = (pk(10), pk(11));
    let pools = vec![
        make_pool(pk(1), mint_a, mint_b),
        make_pool(pk(2), mint_a, mint_b),
    ];
    let metadata = MockMetadataRepo::with(vec![
        (mint_a, make_metadata(mint_a, "SOL")),
        (mint_b, make_metadata(mint_b, "USDC")),
    ]);
    let feed = MockSignalRepo::feed(
        Ok(None),
        Ok(vec![
            make_signal_record(10, pk(1)),
            make_signal_record(11, pk(2)),
        ]),
    );
    let source = SignalsSource::new(Arc::new(feed), enricher(pools, metadata));
    let (tx, _) = broadcast::channel::<Arc<EnrichedSignal>>(8);
    let mut subscribers: Vec<_> = (0..3).map(|_| tx.subscribe()).collect();

    poll_once(&source, &tx, Some(LiveWatermark::anchored(9))).await;

    for rx in &mut subscribers {
        for expected_id in [10, 11] {
            let signal = rx.try_recv().expect("every subscriber gets every signal");
            assert_eq!(signal.record.id, expected_id);
            assert_eq!(symbol(&signal.token_a), Some("SOL"));
            assert_eq!(symbol(&signal.token_b), Some("USDC"));
        }
    }
}

/// Delivering beats decorating: when the enrichment fails, the signals still go out, bare,
/// and the watermark still advances.
#[tokio::test]
async fn a_failed_enrichment_still_delivers_the_signals_bare() {
    let feed = MockSignalRepo::feed(Ok(None), Ok(vec![make_signal_record(10, pk(1))]));
    let failing = enricher(
        vec![make_pool(pk(1), pk(10), pk(11))],
        MockMetadataRepo::failing(),
    );
    let source = SignalsSource::new(Arc::new(feed), failing);
    let (tx, mut rx) = broadcast::channel::<Arc<EnrichedSignal>>(8);

    let watermark = poll_once(&source, &tx, Some(LiveWatermark::anchored(9)))
        .await
        .expect("kept");

    assert_eq!(watermark.floor(), 10);
    let signal = rx.try_recv().expect("the signal goes out bare");
    assert_eq!(signal.record.id, 10);
    assert_eq!(symbol(&signal.token_a), None);
}
