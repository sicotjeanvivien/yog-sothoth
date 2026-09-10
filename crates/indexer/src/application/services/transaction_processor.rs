use solana_rpc_client_api::response::transaction::Signature;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use yog_core::{
    application::extraction::{
        ExtractionDispatcher, ExtractionFailure, ExtractionOutcome, OnChainTransaction,
        discriminator_hex,
    },
    domain::Protocol,
};

use crate::application::services::{EventPersistor, TransactionProcessorMetrics};

/// Core pipeline — receives a transaction a source has already delivered,
/// dispatches it to the appropriate protocol handler, and hands each extracted
/// domain event to the EventPersistor.
///
/// # Why it does not fetch
///
/// It used to, and that was the JSON-RPC acquisition model leaking into the
/// shared half of the pipeline. Fetching is what `logsSubscribe` forces —
/// Yellowstone delivers the transaction whole — so it belongs to the source
/// that needs it, `infra::rpc`, and what arrives here is the same
/// `OnChainTransaction` whichever source produced it.
pub(crate) struct TransactionProcessor {
    extractor: Arc<ExtractionDispatcher>,
    persistor: Arc<EventPersistor>,
}

impl TransactionProcessor {
    pub(crate) fn new(
        extractor: Arc<ExtractionDispatcher>,
        persistor: Arc<EventPersistor>,
    ) -> Self {
        Self {
            extractor,
            persistor,
        }
    }

    /// Handle one transaction delivered by a source.
    ///
    /// Pipeline:
    ///   1. Delegate event extraction to the protocol-specific handler.
    ///   2. Hand each extracted event to the EventPersistor — failures
    ///      on one event never abort the others.
    ///   3. Surface unknown discriminators and extraction failures as
    ///      metrics + structured logs.
    pub(crate) async fn process_transaction(
        &self,
        protocol: Protocol,
        transaction: &OnChainTransaction,
    ) -> anyhow::Result<()> {
        let mut guard = ExitGuard::new(protocol);
        let signature = transaction.position.signature;

        info!(%signature, protocol = %protocol.as_str(), "processing transaction");

        let outcome = match self.extractor.extract(protocol, transaction) {
            Ok(o) => o,
            Err(e) => {
                error!(%signature, error = %e, "extraction failed at transaction level");
                guard.set("extract_failure");
                return Err(e.into());
            }
        };

        self.report_diagnostics(&protocol, &signature, &outcome);

        if outcome.events.is_empty() {
            TransactionProcessorMetrics::record_no_match(&protocol);
            debug!(%signature, "no recognized events in transaction");
            guard.set("no_events");
            return Ok(());
        }

        for event in &outcome.events {
            self.persistor.persist(event).await;
        }

        guard.set("ok");
        Ok(())
    }

    /// Surface unknown discriminators and extraction failures via logs and
    /// metrics. Does not affect persistence.
    fn report_diagnostics(
        &self,
        protocol: &Protocol,
        signature: &Signature,
        outcome: &ExtractionOutcome,
    ) {
        for unknown in &outcome.unknown {
            let hex = discriminator_hex(&unknown.discriminator);
            debug!(
                %signature,
                protocol = %protocol.as_str(),
                discriminator = %hex,
                "unknown anchor event"
            );
            TransactionProcessorMetrics::record_unknown_event(protocol, &hex);
        }

        for failure in &outcome.failures {
            let kind = failure_kind(failure);
            warn!(
                %signature,
                protocol = %protocol.as_str(),
                kind,
                error = %failure,
                "extraction failure"
            );
            TransactionProcessorMetrics::record_extraction_failure(protocol, kind);
        }
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn failure_kind(f: &ExtractionFailure) -> &'static str {
    match f {
        ExtractionFailure::AnchorDecode(_) => "anchor_decode",
        ExtractionFailure::Borsh { .. } => "borsh",
        ExtractionFailure::Translation { .. } => "translation",
        ExtractionFailure::EventIndexOverflow { .. } => "event_index_overflow",
    }
}

// ---------------------------------------------------------------------------
// ExitGuard
// ---------------------------------------------------------------------------

/// RAII guard that records the outcome and duration of `index_transaction`.
///
/// ⚠️ **It no longer spans the fetch**, because the fetch is no longer here.
/// `yog_indexer_index_transaction_duration_seconds` therefore measures
/// extract-and-persist alone — which is what makes it the same measurement on
/// both acquisition paths, and so worth comparing. Fetch latency has its own
/// histogram on the one path that has a fetch.
struct ExitGuard {
    protocol: Protocol,
    outcome: Option<&'static str>,
    start: Instant,
}

impl ExitGuard {
    fn new(protocol: Protocol) -> Self {
        TransactionProcessorMetrics::record_entered(&protocol);
        Self {
            protocol,
            outcome: None,
            start: Instant::now(),
        }
    }

    fn set(&mut self, outcome: &'static str) {
        self.outcome = Some(outcome);
    }
}

impl Drop for ExitGuard {
    fn drop(&mut self) {
        let outcome = self.outcome.unwrap_or("unknown_exit");
        TransactionProcessorMetrics::record_exited(&self.protocol, outcome);
        TransactionProcessorMetrics::record_index_tx_duration(
            &self.protocol,
            outcome,
            self.start.elapsed().as_secs_f64(),
        );
    }
}
