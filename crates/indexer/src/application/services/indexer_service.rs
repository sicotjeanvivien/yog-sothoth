use solana_commitment_config::CommitmentConfig;
use solana_rpc_client::nonblocking::rpc_client::RpcClient;
use solana_rpc_client_api::{config::RpcTransactionConfig, response::transaction::Signature};
use std::sync::Arc;
use std::time::Instant;
use tokio_retry::{Retry, strategy::FixedInterval};
use tracing::{debug, error, info, warn};
use yog_core::solana_types::{EncodedConfirmedTransactionWithStatusMeta, UiTransactionEncoding};
use yog_core::{
    domain::Protocol,
    protocols::{
        PoolIndexer,
        extraction::{ExtractionFailure, ExtractionOutcome, discriminator_hex},
        meteora::{MeteoraDammV1, MeteoraDammV2, MeteoraDlmm},
    },
};

use crate::application::services::{EventPersistor, IndexerServiceMetrics, errors::FetchError};

/// Core pipeline — receives a signature, fetches the full transaction,
/// dispatches to the appropriate protocol handler, hands each extracted
/// domain event to the EventPersistor.
pub(crate) struct IndexerService {
    persistor: Arc<EventPersistor>,
    rpc_client: Arc<RpcClient>,
}

impl IndexerService {
    pub(crate) fn new(persistor: Arc<EventPersistor>, rpc_client: Arc<RpcClient>) -> Self {
        Self {
            persistor,
            rpc_client,
        }
    }

    /// Handle a transaction signature received from the WebSocket.
    ///
    /// Pipeline:
    ///   1. Fetch the full transaction from RPC.
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

        let tx = match self.fetch_transaction(&protocol, signature).await {
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

        let indexer = protocol_indexer(&protocol);

        let outcome = match indexer.extract_events(&tx) {
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

    /// Fetch a confirmed transaction by signature from the RPC.
    async fn fetch_transaction(
        &self,
        protocol: &Protocol,
        signature: Signature,
    ) -> Result<EncodedConfirmedTransactionWithStatusMeta, FetchError> {
        let config = RpcTransactionConfig {
            encoding: Some(UiTransactionEncoding::JsonParsed),
            commitment: Some(CommitmentConfig::confirmed()),
            max_supported_transaction_version: Some(0),
        };

        let strategy = FixedInterval::from_millis(500).take(5);
        let result = Retry::spawn(strategy, || async {
            self.rpc_client
                .get_transaction_with_config(&signature, config)
                .await
                .map_err(|e| e.to_string())
        })
        .await;

        let start = Instant::now();
        IndexerServiceMetrics::record_fetch_duration(protocol, start.elapsed().as_secs_f64());

        result.map_err(FetchError::from_rpc_string)
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

fn protocol_indexer(protocol: &Protocol) -> Arc<dyn PoolIndexer> {
    match protocol {
        Protocol::MeteoraDammV2 => Arc::new(MeteoraDammV2::new()),
        Protocol::MeteoraDammV1 => Arc::new(MeteoraDammV1::new()),
        Protocol::MeteoraDlmm => Arc::new(MeteoraDlmm::new()),
    }
}

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
