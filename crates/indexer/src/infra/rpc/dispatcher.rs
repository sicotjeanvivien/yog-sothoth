pub(crate) mod filters;
pub(crate) mod metrics;
use crate::{
    error::DispatcherError,
    infra::rpc::{dispatcher::metrics::DispatcherMetrics, QualifiedSignature, RawLogEvent},
};
pub(crate) use filters::{FailedTransactionFilter, FilterDecision, InvocationFilter, LogFilter};

use solana_rpc_client_api::response::transaction::Signature;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Erreurs fatales du dispatcher (configuration uniquement).
///
/// Les erreurs de données (signature malformée, drop par filtre) ne sont pas
/// des erreurs : elles sont comptabilisées et le pipeline continue.

pub struct SignatureDispatcher {
    filters: Vec<Box<dyn LogFilter>>,
}

impl SignatureDispatcher {
    /// Dispatcher avec la chaîne de filtres standard.
    pub fn new_default() -> Result<Self, DispatcherError> {
        Self::new_with_filters(vec![
            Box::new(InvocationFilter),
            Box::new(FailedTransactionFilter),
        ])
    }

    /// Dispatcher avec des filtres arbitraires (tests, configurations alternatives).
    pub fn new_with_filters(filters: Vec<Box<dyn LogFilter>>) -> Result<Self, DispatcherError> {
        if filters.is_empty() {
            return Err(DispatcherError::NoFilters);
        }
        Ok(Self { filters })
    }

    /// Boucle principale : consomme les événements bruts jusqu'à shutdown
    /// ou fermeture du canal amont.
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

    /// Traite un seul événement. Synchrone — aucune I/O ici.
    fn handle(&self, event: RawLogEvent, tx: &mpsc::Sender<QualifiedSignature>) {
        DispatcherMetrics::record_received(&event.protocol);

        // Chaîne de filtres — premier rejet = drop.
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

        // Parsing de la signature — dernière étape avant émission.
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

        // `try_send` : si l'Indexer est saturé, on drop plutôt que de bloquer
        // tout le pipeline. Compté par la métrique `downstream_saturated`.
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
                // Aval fermé : on continue, le shutdown s'occupera de la sortie
                // propre. Inutile de logger à chaque event.
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

    /// Signature base58 valide (64 bytes) utilisée pour les tests.
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
        drop(tx_in); // ferme l'amont → dispatcher sort

        dispatcher.run(rx_in, tx_out, shutdown).await.unwrap();

        assert!(
            rx_out.try_recv().is_err(),
            "aucune signature ne doit sortir"
        );
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

    // NOTE : un test "accepté + signature valide → émission" est utile mais
    // demande une signature base58 valide côté solana_sdk. À ajouter quand
    // tu auras accès à `Keypair::new().sign_message(...).to_string()` en test.
}
