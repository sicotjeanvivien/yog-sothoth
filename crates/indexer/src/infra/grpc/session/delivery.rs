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
            // Skip-and-log per transaction: a malformed message must not stop the stream.
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

        // ⚠️ Back-pressure, not dropping (unlike the JSON-RPC dispatcher): a streamed
        // transaction comes once and cannot be asked for again.
        let ingested = match self.downstream.try_send(ingested) {
            Ok(()) => {
                GrpcListenerMetrics::record_emitted();
                return SessionState::Open;
            }
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
                if sent.is_err() {
                    return SessionState::DownstreamClosed;
                }
                GrpcListenerMetrics::record_emitted();
                SessionState::Open
            }
            _ = self.shutdown.cancelled() => SessionState::ShutdownRequested,
        }
    }
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
