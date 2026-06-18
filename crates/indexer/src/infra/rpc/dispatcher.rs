mod filters;
mod metrics;
use crate::{
    error::DispatcherError,
    infra::rpc::{QualifiedSignature, RawLogEvent},
};
pub(crate) use filters::{FailedTransactionFilter, FilterDecision, InvocationFilter, LogFilter};
pub(crate) use metrics::DispatcherMetrics;

use solana_rpc_client_api::response::transaction::Signature;
use std::str::FromStr;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Fatal dispatcher errors (configuration only).
///
/// Data errors (malformed signature, filter drop) are not errors:
/// they are recorded as metrics and the pipeline keeps going.
pub(crate) struct SignatureDispatcher {
    filters: Vec<Box<dyn LogFilter>>,
}

impl SignatureDispatcher {
    /// Dispatcher with the standard filter chain.
    pub(crate) fn new_default() -> Result<Self, DispatcherError> {
        Self::new_with_filters(vec![
            Box::new(InvocationFilter),
            Box::new(FailedTransactionFilter),
        ])
    }

    /// Dispatcher with arbitrary filters (tests, alternative configurations).
    pub(crate) fn new_with_filters(
        filters: Vec<Box<dyn LogFilter>>,
    ) -> Result<Self, DispatcherError> {
        if filters.is_empty() {
            return Err(DispatcherError::NoFilters);
        }
        Ok(Self { filters })
    }

    /// Main loop: consumes raw events until shutdown
    /// or upstream channel closure.
    pub(crate) async fn run(
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
            protocol: event.protocol,
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
#[path = "dispatcher_tests.rs"]
mod tests;
