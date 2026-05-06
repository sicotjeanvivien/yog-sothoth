pub(crate) mod filters;
pub(crate) mod metrics;
use crate::{
    error::DispatcherError,
    infra::rpc::{QualifiedSignature, RawLogEvent, dispatcher::metrics::DispatcherMetrics},
};
pub(crate) use filters::{FailedTransactionFilter, FilterDecision, InvocationFilter, LogFilter};

use solana_rpc_client_api::response::transaction::Signature;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Fatal dispatcher errors (configuration only).
///
/// Data errors (malformed signature, filter drop) are not errors:
/// they are recorded as metrics and the pipeline keeps going.

pub struct SignatureDispatcher {
    filters: Vec<Box<dyn LogFilter>>,
}

impl SignatureDispatcher {
    /// Dispatcher with the standard filter chain.
    pub fn new_default() -> Result<Self, DispatcherError> {
        Self::new_with_filters(vec![
            Box::new(InvocationFilter),
            Box::new(FailedTransactionFilter),
        ])
    }

    /// Dispatcher with arbitrary filters (tests, alternative configurations).
    pub fn new_with_filters(filters: Vec<Box<dyn LogFilter>>) -> Result<Self, DispatcherError> {
        if filters.is_empty() {
            return Err(DispatcherError::NoFilters);
        }
        Ok(Self { filters })
    }

    /// Main loop: consumes raw events until shutdown
    /// or upstream channel closure.
    pub async fn run(
        self,
        mut rx: mpsc::Receiver<RawLogEvent>,
        tx: mpsc::Sender<QualifiedSignature>,
        shutdown: CancellationToken,
    ) -> Result<(), DispatcherError> {
        info!(filters = self.filters.len(), "SignatureDispatcher started");

        loop {
            tokio::select! {
                maybe_event = rx.recv() => {
                    match maybe_event {
                        Some(event) => self.handle(event, &tx),
                        None => {
                            info!("upstream channel closed — dispatcher stopping");
                            return Ok(());
                        }
                    }
                }
                _ = shutdown.cancelled() => {
                    info!("shutdown requested — dispatcher stopping");
                    return Ok(());
                }
            }
        }
    }

    /// Handles a single event. Synchronous — no I/O here.
    fn handle(&self, event: RawLogEvent, tx: &mpsc::Sender<QualifiedSignature>) {
        DispatcherMetrics::record_received(&event.protocol);

        // Filter chain — first rejection = drop.
        for filter in &self.filters {
            match filter.accept(&event) {
                FilterDecision::Accept => continue,
                FilterDecision::Reject { reason } => {
                    debug!(
                        protocol = %event.protocol.as_str(),
                        signature = %event.signature,
                        filter = filter.name(),
                        reason,
                        "event rejected"
                    );
                    DispatcherMetrics::record_rejected(&event.protocol, filter.name(), reason);
                    return;
                }
            }
        }

        // Signature parsing — last step before emission.
        let signature = match Signature::from_str(&event.signature) {
            Ok(sig) => sig,
            Err(e) => {
                debug!(
                    protocol = %event.protocol.as_str(),
                    raw = %event.signature,
                    error = %e,
                    "signature parse failed"
                );
                DispatcherMetrics::record_malformed(&event.protocol);
                return;
            }
        };

        let qualified = QualifiedSignature {
            protocol: event.protocol.clone(),
            signature,
        };

        // `try_send`: if the Indexer is saturated, drop rather than block
        // the whole pipeline. Tracked by the `downstream_saturated` metric.
        match tx.try_send(qualified) {
            Ok(()) => {
                DispatcherMetrics::record_emitted(&event.protocol);
            }
            Err(mpsc::error::TrySendError::Full(dropped)) => {
                debug!(
                    protocol = %dropped.protocol.as_str(),
                    signature = %dropped.signature,
                    "downstream saturated — signature dropped"
                );
                DispatcherMetrics::record_downstream_saturated(&dropped.protocol);
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                // Downstream closed: keep going, shutdown will handle the
                // clean exit. No need to log on every event.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
}
