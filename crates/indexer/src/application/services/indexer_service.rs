use solana_rpc_client_api::response::transaction::Signature;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use yog_core::{
    domain::Protocol,
    protocols::{
        EventExtractor,
        extraction::{ExtractionFailure, ExtractionOutcome, discriminator_hex},
    },
};

use crate::{
    application::services::{EventPersistor, IndexerServiceMetrics},
    infra::rpc::{FetchError, TransactionFetcher},
};

/// Core pipeline — receives a signature, fetches the full transaction via
/// the TransactionFetcher, dispatches to the appropriate protocol handler,
/// hands each extracted domain event to the EventPersistor.
pub(crate) struct IndexerService {
    fetcher: Arc<TransactionFetcher>,
    extractor: Arc<EventExtractor>,
    persistor: Arc<EventPersistor>,
}

impl IndexerService {
    pub(crate) fn new(
        fetcher: Arc<TransactionFetcher>,
        extractor: Arc<EventExtractor>,
        persistor: Arc<EventPersistor>,
    ) -> Self {
        Self {
            fetcher,
            extractor,
            persistor,
        }
    }

    /// Handle a transaction signature received from the WebSocket.
    ///
    /// Pipeline:
    ///   1. Fetch the full transaction via the TransactionFetcher.
    ///   2. Delegate event extraction to the protocol-specific handler.
    ///   3. Hand each extracted event to the EventPersistor — failures
    ///      on one event never abort the others.
    ///   4. Surface unknown discriminators and extraction failures as
    ///      metrics + structured logs.
    pub(crate) async fn index_transaction(
        &self,
        protocol: Protocol,
        signature: Signature,
    ) -> anyhow::Result<()> {
        let mut guard = ExitGuard::new(protocol);

        info!(%signature, protocol = %protocol.as_str(), "received signature");

        let start = Instant::now();
        let result = self.fetcher.fetch(signature).await;
        IndexerServiceMetrics::record_fetch_duration(&protocol, start.elapsed().as_secs_f64());

        let tx = match result {
            Ok(tx) => tx,
            Err(FetchError::NotFound) => {
                IndexerServiceMetrics::record_fetch_not_found(&protocol);
                guard.set("fetch_not_found");
                return Ok(());
            }
            Err(e) => {
                IndexerServiceMetrics::record_fetch_failure(&protocol, e.metric_label());
                guard.set("fetch_failure");
                return Err(e.into());
            }
        };

        let outcome = match self.extractor.extract(protocol, &tx) {
            Ok(o) => o,
            Err(e) => {
                error!(%signature, error = %e, "extraction failed at transaction level");
                guard.set("extract_failure");
                return Err(e.into());
            }
        };

        self.report_diagnostics(&protocol, &signature, &outcome);

        if outcome.events.is_empty() {
            IndexerServiceMetrics::record_no_match(&protocol);
            debug!(%signature, "no recognized events in transaction");
            guard.set("no_events");
            return Ok(());
        }

        for event in &outcome.events {
            self.persistor.persist(&protocol, event).await;
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
            IndexerServiceMetrics::record_unknown_event(protocol, &hex);
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
            IndexerServiceMetrics::record_extraction_failure(protocol, kind);
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
    }
}

// ---------------------------------------------------------------------------
// ExitGuard
// ---------------------------------------------------------------------------

/// RAII guard that records the outcome and duration of `index_transaction`.
struct ExitGuard {
    protocol: Protocol,
    outcome: Option<&'static str>,
    start: Instant,
}

impl ExitGuard {
    fn new(protocol: Protocol) -> Self {
        IndexerServiceMetrics::record_entered(&protocol);
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
        IndexerServiceMetrics::record_exited(&self.protocol, outcome);
        IndexerServiceMetrics::record_index_tx_duration(
            &self.protocol,
            outcome,
            self.start.elapsed().as_secs_f64(),
        );
    }
}
