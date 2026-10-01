//! Handing a timestamped transaction to the pipeline, without losing it.

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yog_core::CoreError;

use crate::{
    application::source::IngestedTransaction,
    infra::grpc::{
        metrics::{DropReason, GrpcListenerMetrics},
        transaction_adapter::from_grpc,
    },
};

use super::{SessionState, log, slot_progress::PendingTransaction};

/// The downstream end of a session.
pub(super) struct Delivery {
    downstream: mpsc::Sender<IngestedTransaction>,
    /// So that a wait on a full consumer is interruptible.
    shutdown: CancellationToken,
}

impl Delivery {
    pub(super) fn new(
        downstream: mpsc::Sender<IngestedTransaction>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            downstream,
            shutdown,
        }
    }

    /// Translate one transaction and hand it downstream.
    pub(super) async fn deliver(
        &self,
        pending: PendingTransaction,
        at: DateTime<Utc>,
    ) -> SessionState {
        let transaction = match from_grpc(&pending.update, at) {
            Ok(transaction) => transaction,
            // Skip-and-log, per transaction: a malformed message must not stop
            // the stream. `from_grpc`'s doc-comment lists what can appear here.
            Err(error) => {
                GrpcListenerMetrics::record_dropped(drop_reason(&error));
                log::untranslatable(pending.update.slot, &error);
                return SessionState::Open;
            }
        };

        let ingested = IngestedTransaction {
            protocol: pending.protocol,
            transaction,
        };

        // ⚠️ Back-pressure, not dropping — the opposite of the JSON-RPC
        // dispatcher. There a dropped signature can be asked for again; here
        // the transaction came once, over a stream billed by the byte. So the
        // stream is slowed to the consumer's speed, bounded only by shutdown.
        let ingested = match self.downstream.try_send(ingested) {
            Ok(()) => return emitted(),
            Err(mpsc::error::TrySendError::Closed(_)) => return SessionState::DownstreamClosed,
            Err(mpsc::error::TrySendError::Full(ingested)) => ingested,
        };

        GrpcListenerMetrics::record_downstream_full();
        log::downstream_full();
        self.wait_for_room(ingested).await
    }

    /// Wait until the consumer takes `ingested`, or until shutdown.
    async fn wait_for_room(&self, ingested: IngestedTransaction) -> SessionState {
        tokio::select! {
            sent = self.downstream.send(ingested) => {
                if sent.is_ok() { emitted() } else { SessionState::DownstreamClosed }
            }
            _ = self.shutdown.cancelled() => SessionState::ShutdownRequested,
        }
    }
}

/// A transaction left for the pipeline.
fn emitted() -> SessionState {
    GrpcListenerMetrics::record_emitted();
    SessionState::Open
}

/// The reason label for a translation failure: a bounded set, so the counter
/// does not become a cardinality problem the day an error quotes a signature.
fn drop_reason(error: &CoreError) -> DropReason {
    match error {
        CoreError::MissingField { .. } => DropReason::MissingField,
        CoreError::ParseError { .. } => DropReason::ParseError,
        // `from_grpc` returns only the two above; the catch-all keeps a future
        // variant countable.
        _ => DropReason::OtherMalformation,
    }
}
