//! One subscription's worth of state, and what each update does to it.
//!
//! The listener owns *connecting*; this owns **what an update means**, which
//! is pure state and channels, so every branch is reachable from a test with
//! no wire. A session lives exactly as long as one subscription.
//!
//! This file routes each update. The rest has its own module:
//!
//! - `slot_progress` — which slots are finished: when a transaction leaves,
//!   and where a reconnection resumes;
//! - `delivery` — handing a transaction to the pipeline under back-pressure;
//! - `ping_answer` — answering server pings;
//! - `log` — the lines a session writes.

mod delivery;
mod log;
mod ping_answer;
mod slot_progress;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use yellowstone_grpc_proto::prelude::{
    SubscribeRequest, SubscribeUpdate, SubscribeUpdateBlockMeta, SubscribeUpdateTransaction,
    subscribe_update::UpdateOneof,
};
use yog_core::domain::Protocol;

use crate::application::source::IngestedTransaction;

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
    /// The consumer is gone. Nothing downstream will ever take a transaction
    /// again, so there is no point holding the stream open — and no point
    /// retrying either, which is why the listener treats this as a clean stop
    /// rather than a failure.
    DownstreamClosed,
    /// Shutdown was requested **while waiting** for a full consumer.
    ///
    /// ⚠️ This variant exists because the wait happens inside `handle`, which
    /// the listener drives from the *body* of a `select!` arm and not as a
    /// branch: while it is parked, `shutdown.cancelled()` is not being polled.
    /// A consumer that stalls without dropping its receiver would otherwise
    /// make the listener ignore graceful shutdown for as long as the stall
    /// lasts. Found in review, 9 September 2026.
    ShutdownRequested,
}

/// The state one subscription accumulates.
pub(super) struct StreamSession {
    slots: SlotProgress,
    delivery: Delivery,
    pings: PingAnswer,
    /// Whether any **data** came off the stream — a transaction or a
    /// block-meta. Not "any message": see [`Self::received_data`].
    received_data: bool,
    /// How many block-metas came off the stream — see
    /// [`Self::block_metas_received`].
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

    /// Whether this session received any **data** — a transaction or a
    /// block-meta.
    ///
    /// What the listener does with it: a connection that opened and closed
    /// having delivered no data is a **failing attempt**, not the churn of a
    /// long-lived stream, so it counts against the retry budget. Without that
    /// distinction a server that accepts `subscribe` and closes at once — an
    /// exhausted quota, a token refused at stream level — is retried for ever
    /// at one attempt per second, and the budget never fires.
    ///
    /// ⚠️ **A ping does not count**, and reading "any message" here would undo
    /// the whole rule: Yellowstone servers ping shortly after `subscribe`, so a
    /// stream that pings once and closes would land in the churn arm, reset the
    /// counter, and be redialled once a second for ever — exactly the failure
    /// the distinction was introduced to stop. Found in review, 9 September
    /// 2026, one day after the rule itself.
    pub(super) fn received_data(&self) -> bool {
        self.received_data
    }

    /// How many block-metas this session took off the stream, with a block
    /// time or not. The listener's stall clock restarts when it moves.
    ///
    /// ⚠️ Never moved by a ping: pings come from a task of their own on the
    /// server, so they keep coming after its data path has stopped.
    pub(super) fn block_metas_received(&self) -> u64 {
        self.block_metas
    }

    /// Where a reconnection should resume from, or `None` when nothing arrived
    /// — see `SlotProgress::resume_from`.
    pub(super) fn resume_from(&self) -> Option<u64> {
        self.slots.resume_from()
    }

    /// Take one update off the stream.
    ///
    /// Every failure below is per-update: counted, logged, stepped over. The
    /// one thing that ends a session is the consumer disappearing.
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
                self.on_block_meta(meta).await
            }
            Some(UpdateOneof::Ping(_)) => {
                // Not data: `received_data` stays false — see its doc-comment.
                GrpcListenerMetrics::record_update(UpdateKind::Ping);
                self.pings.answer();
                SessionState::Open
            }
            Some(UpdateOneof::Pong(_)) => {
                GrpcListenerMetrics::record_update(UpdateKind::Pong);
                SessionState::Open
            }
            // Anything the subscription did not ask for, and the absent oneof a
            // future schema could introduce. Counted rather than ignored: a
            // non-zero `other` means the request and this reader disagree about
            // what was subscribed to, which nothing else would say.
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
        // A transaction that matched no protocol filter cannot be routed: the
        // pipeline is per-protocol all the way down. Dropped and counted, since
        // it means the request and this reader disagree — and re-deriving the
        // protocol from the account keys here would paper over exactly that.
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

    async fn on_block_meta(&mut self, meta: SubscribeUpdateBlockMeta) -> SessionState {
        for resolved in self.slots.on_block_meta(meta) {
            match self.delivery.deliver(resolved.payload, resolved.at).await {
                SessionState::Open => {}
                // Either end of the session: the rest of the batch dies with the
                // buffer, and `resume_from` is what asks for it again.
                ended => return ended,
            }
        }
        SessionState::Open
    }
}

#[cfg(test)]
#[path = "tests/session_tests.rs"]
mod tests;
