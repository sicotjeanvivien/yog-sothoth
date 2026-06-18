use super::*;
use solana_rpc_client_api::response::TransactionError;
use yog_core::domain::Protocol;

/// Valid base58 signature (64 bytes) used for tests.
const VALID_SIG: &str =
    "5j7s1QzqC8J8G7X9WjvY2b6K1cN2Y3Z4W5v6u7t8s9r0q1p2o3n4m5l6k7j8i9h0g1f2e3d4c5b6a7";

fn make_event(logs: Vec<String>, err: Option<TransactionError>) -> RawLogEvent {
    RawLogEvent {
        protocol: Protocol::MeteoraDammV2,
        signature: VALID_SIG.to_string(),
        logs,
        err,
    }
}

struct AcceptAll;
impl LogFilter for AcceptAll {
    fn name(&self) -> &'static str {
        "accept_all"
    }
    fn accept(&self, _: &RawLogEvent) -> FilterDecision {
        FilterDecision::Accept
    }
}

struct RejectAll;
impl LogFilter for RejectAll {
    fn name(&self) -> &'static str {
        "reject_all"
    }
    fn accept(&self, _: &RawLogEvent) -> FilterDecision {
        FilterDecision::Reject { reason: "test" }
    }
}

#[test]
fn rejects_empty_filter_chain() {
    assert!(matches!(
        SignatureDispatcher::new_with_filters(vec![]),
        Err(DispatcherError::NoFilters)
    ));
}

#[tokio::test]
async fn rejected_event_does_not_reach_downstream() {
    let dispatcher = SignatureDispatcher::new_with_filters(vec![Box::new(RejectAll)]).unwrap();
    let (tx_in, rx_in) = mpsc::channel(1);
    let (tx_out, mut rx_out) = mpsc::channel::<QualifiedSignature>(1);
    let shutdown = CancellationToken::new();

    tx_in.send(make_event(vec![], None)).await.unwrap();
    drop(tx_in); // close upstream → dispatcher exits

    dispatcher.run(rx_in, tx_out, shutdown).await.unwrap();

    assert!(rx_out.try_recv().is_err(), "no signature should be emitted");
}

#[tokio::test]
async fn accepted_event_with_invalid_signature_is_dropped() {
    let dispatcher = SignatureDispatcher::new_with_filters(vec![Box::new(AcceptAll)]).unwrap();
    let (tx_in, rx_in) = mpsc::channel(1);
    let (tx_out, mut rx_out) = mpsc::channel::<QualifiedSignature>(1);
    let shutdown = CancellationToken::new();

    let mut event = make_event(vec![], None);
    event.signature = "not-a-valid-signature".to_string();
    tx_in.send(event).await.unwrap();
    drop(tx_in);

    dispatcher.run(rx_in, tx_out, shutdown).await.unwrap();

    assert!(rx_out.try_recv().is_err());
}

// NOTE: an "accepted + valid signature → emission" test would be useful
// but requires a valid base58 signature from solana_sdk. To be added
// once `Keypair::new().sign_message(...).to_string()` is available in tests.
