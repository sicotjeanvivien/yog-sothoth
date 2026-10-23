//! One subscription's state, and what each update does to it: pure state and channels, no wire.
//! `slot_progress` decides when a transaction leaves and where a reconnection resumes, `delivery`
//! hands it on under back-pressure, `ping_answer` answers server pings.

mod delivery;
mod log;
mod ping_answer;
mod slot_progress;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yellowstone_grpc_proto::prelude::{
    SubscribeRequest, SubscribeUpdate, SubscribeUpdateTransaction, subscribe_update::UpdateOneof,
};
use yog_core::domain::Protocol;

use crate::{
    application::source::IngestedTransaction, infra::grpc::slot_timestamp_buffer::Resolved,
};

use super::{
    metrics::{DropReason, GrpcListenerMetrics, UpdateKind},
    subscription::protocol_of,
};

use delivery::Delivery;
use ping_answer::PingAnswer;
use slot_progress::{PendingTransaction, SlotProgress};

/// Why a session ended, or that it has not.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum SessionState {
    /// Keep reading.
    Open,
    /// The consumer is gone; the listener treats this as a clean stop, not a failure to retry.
    DownstreamClosed,
    /// Shutdown was requested **while waiting** for a full consumer.
    /// ⚠️ `handle` runs in a `select!` arm's body, where `shutdown.cancelled()` is not polled:
    /// without this, a stalled consumer would hold off graceful shutdown.
    ShutdownRequested,
}

/// The state one subscription accumulates.
pub(super) struct StreamSession {
    slots: SlotProgress,
    delivery: Delivery,
    pings: PingAnswer,
    /// Whether a data message came off the stream; see [`Self::received_data`].
    received_data: bool,
    /// See [`Self::block_metas_received`].
    block_metas: u64,
}

impl StreamSession {
    pub(super) fn new(
        downstream: mpsc::Sender<IngestedTransaction>,
        outbound: mpsc::Sender<SubscribeRequest>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            slots: SlotProgress::new(),
            delivery: Delivery::new(downstream, shutdown),
            pings: PingAnswer::new(outbound),
            received_data: false,
            block_metas: 0,
        }
    }

    /// Whether a data message came; a session with none is charged to the retry budget.
    /// ⚠️ A ping does not count: servers ping right after `subscribe`, so a stream that pings and
    /// closes would otherwise reset the budget and be redialled once a second for ever.
    pub(super) fn received_data(&self) -> bool {
        self.received_data
    }

    /// How many block-metas this session took off the stream; the stall clock restarts when it moves.
    /// ⚠️ Never moved by a ping: the server keeps pinging after its data path has stopped.
    pub(super) fn block_metas_received(&self) -> u64 {
        self.block_metas
    }

    /// Where a reconnection should resume from, or `None` when nothing arrived.
    pub(super) fn resume_from(&self) -> Option<u64> {
        self.slots.resume_from()
    }

    /// Take one update off the stream; failures are per-update and stepped over, and only the
    /// consumer disappearing or a shutdown ends the session.
    pub(super) async fn handle(&mut self, update: SubscribeUpdate) -> SessionState {
        let protocol = protocol_of(&update.filters);

        match update.update_oneof {
            Some(UpdateOneof::Transaction(transaction)) => {
                GrpcListenerMetrics::record_update(UpdateKind::Transaction);
                self.received_data = true;
                self.on_transaction(protocol, transaction).await
            }
            Some(UpdateOneof::BlockMeta(meta)) => {
                GrpcListenerMetrics::record_update(UpdateKind::BlockMeta);
                self.received_data = true;
                self.block_metas += 1;
                let released = self.slots.on_block_meta(meta);
                self.deliver_all(released).await
            }
            Some(UpdateOneof::Slot(status)) => {
                GrpcListenerMetrics::record_update(UpdateKind::Slot);
                self.received_data = true;
                let released = self.slots.on_slot_status(status);
                self.deliver_all(released).await
            }
            Some(UpdateOneof::Ping(_)) => {
                GrpcListenerMetrics::record_update(UpdateKind::Ping);
                self.pings.answer();
                SessionState::Open
            }
            Some(UpdateOneof::Pong(_)) => {
                GrpcListenerMetrics::record_update(UpdateKind::Pong);
                SessionState::Open
            }
            // Unasked or unknown updates are counted: a non-zero `other` says the request and
            // this reader disagree.
            _ => {
                GrpcListenerMetrics::record_update(UpdateKind::Other);
                SessionState::Open
            }
        }
    }

    async fn on_transaction(
        &mut self,
        protocol: Option<Protocol>,
        update: SubscribeUpdateTransaction,
    ) -> SessionState {
        // ⚠️ Dropped and counted, not re-derived from the account keys: that would hide a request
        // and a reader that disagree.
        let Some(protocol) = protocol else {
            GrpcListenerMetrics::record_dropped(DropReason::Unroutable);
            log::unroutable(update.slot);
            return SessionState::Open;
        };

        match self
            .slots
            .on_transaction(PendingTransaction { protocol, update })
        {
            Some(resolved) => self.delivery.deliver(resolved.payload, resolved.at).await,
            None => SessionState::Open,
        }
    }

    async fn deliver_all(&mut self, released: Vec<Resolved<PendingTransaction>>) -> SessionState {
        for resolved in released {
            match self.delivery.deliver(resolved.payload, resolved.at).await {
                SessionState::Open => {}
                // The rest of the batch dies with the buffer; `resume_from` asks for it again.
                ended => return ended,
            }
        }
        SessionState::Open
    }
}

#[cfg(test)]
#[path = "tests/session_tests.rs"]
mod tests;
