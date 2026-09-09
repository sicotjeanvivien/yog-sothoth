//! One subscription's worth of state, and what each update does to it.
//!
//! Split from `listener` because the two answer different questions and only
//! one of them can be tested here. The listener owns *connecting* — an address,
//! TLS, a retry budget — and nothing local can exercise that. This owns
//! **what an update means**, which is pure state and a channel, so every
//! branch below is reachable from a test with no wire at all.
//!
//! A session lives exactly as long as one subscription: it is built when the
//! stream opens and dropped when it closes.
//!
//! # ⚠️ Why the buffer belongs to the session and not to the listener
//!
//! This is the reconnection decision of `03 - active/listener-grpc-yellowstone.md`,
//! and it is expressed by ownership rather than by a method someone has to
//! remember to call.
//!
//! The buffer evicts the oldest pending slot, which is right on a stream that
//! delivers slots in order. A `from_slot` replay is exactly the case where that
//! is false: the older slots arrive **last**, so a buffer still holding the
//! pre-cut backlog would evict each replayed arrival at the moment it enters —
//! a reconnection losing precisely the data it reconnected to recover, silently
//! but for the counter. A buffer that cannot outlive its subscription cannot
//! have that bug.
//!
//! What is dropped with the old session is not lost twice over: a payload
//! pending at the moment of the cut is either re-delivered by the replay, or
//! was already beyond saving.

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tracing::{debug, warn};
use yellowstone_grpc_proto::prelude::{
    SubscribeRequest, SubscribeRequestPing, SubscribeUpdate, SubscribeUpdateBlockMeta,
    SubscribeUpdateTransaction, subscribe_update::UpdateOneof,
};
use yog_core::{CoreError, domain::Protocol};

use super::{
    ingested_transaction::IngestedTransaction,
    metrics::{GrpcListenerMetrics, UpdateKind},
    slot_timestamp_buffer::SlotTimestampBuffer,
    subscription::protocol_of,
    transaction_adapter::from_grpc,
};

/// A transaction waiting for its slot's block time.
///
/// Holds the whole update: `from_grpc` reads `slot` off it, and translating
/// before the instant is known would mean inventing one.
struct PendingTransaction {
    protocol: Protocol,
    update: SubscribeUpdateTransaction,
}

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
}

/// The state one subscription accumulates.
pub(super) struct StreamSession {
    buffer: SlotTimestampBuffer<PendingTransaction>,
    downstream: mpsc::Sender<IngestedTransaction>,
    /// The outbound half of the bidirectional stream, for answering pings.
    outbound: mpsc::Sender<SubscribeRequest>,
    /// The request this session is subscribed with — resent, unchanged, as the
    /// body of a ping answer. See [`Self::answer_ping`].
    request: SubscribeRequest,
    /// The highest slot any update has mentioned, which is where a
    /// reconnection asks to resume from.
    highest_slot: Option<u64>,
}

impl StreamSession {
    pub(super) fn new(
        downstream: mpsc::Sender<IngestedTransaction>,
        outbound: mpsc::Sender<SubscribeRequest>,
        request: SubscribeRequest,
    ) -> Self {
        Self {
            buffer: SlotTimestampBuffer::new(),
            downstream,
            outbound,
            request,
            highest_slot: None,
        }
    }

    /// Where a reconnection should resume from, or `None` if nothing arrived.
    pub(super) fn highest_slot(&self) -> Option<u64> {
        self.highest_slot
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
                self.on_transaction(protocol, transaction).await
            }
            Some(UpdateOneof::BlockMeta(meta)) => {
                GrpcListenerMetrics::record_update(UpdateKind::BlockMeta);
                self.on_block_meta(meta).await
            }
            Some(UpdateOneof::Ping(_)) => {
                GrpcListenerMetrics::record_update(UpdateKind::Ping);
                self.answer_ping().await;
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
        let slot = update.slot;
        self.see_slot(slot);

        // A transaction that matched no protocol filter cannot be routed: the
        // pipeline is per-protocol all the way down. Dropped and counted, since
        // it means the request and this reader disagree — and re-deriving the
        // protocol from the account keys here would paper over exactly that.
        let Some(protocol) = protocol else {
            GrpcListenerMetrics::record_adapter_failure("unroutable");
            warn!(
                slot,
                "transaction update matched no protocol filter — dropping it"
            );
            return SessionState::Open;
        };

        match self
            .buffer
            .on_payload(slot, PendingTransaction { protocol, update })
        {
            Some(resolved) => self.emit(resolved.payload, resolved.at).await,
            // Waiting for its block-meta, or dropped by a bound on the way in —
            // `on_payload` counts and logs that case itself, and the caller does
            // the same thing either way.
            None => SessionState::Open,
        }
    }

    async fn on_block_meta(&mut self, meta: SubscribeUpdateBlockMeta) -> SessionState {
        let slot = meta.slot;
        self.see_slot(slot);

        // ⚠️ `block_time` is optional on the wire, and there is **no** substitute
        // for it: not the receive time, not the neighbouring slot's, not
        // `SubscribeUpdate::created_at` — which is when the server sent the
        // message, not when the block was produced. This column is in every
        // event table's unique key and is the TimescaleDB partitioning column,
        // so a plausible-looking wrong value is one nothing will ever question.
        // The slot is given up instead, under its own eviction reason.
        let Some(at) = meta
            .block_time
            .and_then(|time| DateTime::from_timestamp(time.timestamp, 0))
        else {
            warn!(
                slot,
                "block-meta carried no usable block time — giving up on the slot"
            );
            self.buffer.on_slot_unresolvable(slot);
            return SessionState::Open;
        };

        for resolved in self.buffer.on_block_time(slot, at) {
            if self.emit(resolved.payload, resolved.at).await == SessionState::DownstreamClosed {
                return SessionState::DownstreamClosed;
            }
        }
        SessionState::Open
    }

    /// Translate one transaction and hand it downstream.
    async fn emit(&mut self, pending: PendingTransaction, at: DateTime<Utc>) -> SessionState {
        let transaction = match from_grpc(&pending.update, at) {
            Ok(transaction) => transaction,
            Err(error) => {
                // Skip-and-log, per transaction: a malformed message must not
                // stop the stream. `from_grpc`'s doc-comment lists what can
                // appear here.
                GrpcListenerMetrics::record_adapter_failure(adapter_failure_kind(&error));
                warn!(slot = pending.update.slot, %error, "could not translate a transaction");
                return SessionState::Open;
            }
        };

        let ingested = IngestedTransaction {
            protocol: pending.protocol,
            transaction,
        };

        // ⚠️ Back-pressure, not dropping — the opposite of what the JSON-RPC
        // dispatcher does when its channel is full, and deliberately so. There a
        // dropped signature can be asked for again; here the transaction came
        // once, over a stream billed by the byte, and asking again means the
        // `getTransaction` this path exists to remove. So the stream is slowed
        // to the consumer's speed and the wait is counted.
        match self.downstream.try_send(ingested) {
            Ok(()) => {
                GrpcListenerMetrics::record_emitted();
                SessionState::Open
            }
            Err(mpsc::error::TrySendError::Full(ingested)) => {
                GrpcListenerMetrics::record_downstream_full();
                debug!("downstream is full — slowing the stream to its speed");
                match self.downstream.send(ingested).await {
                    Ok(()) => {
                        GrpcListenerMetrics::record_emitted();
                        SessionState::Open
                    }
                    Err(_) => SessionState::DownstreamClosed,
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => SessionState::DownstreamClosed,
        }
    }

    /// Answer a server ping by **resending the subscription unchanged**.
    ///
    /// # ⚠️ Why the whole request and not a ping on its own
    ///
    /// Because what a Yellowstone server does with a `SubscribeRequest` whose
    /// filters are empty could not be verified from here: the proto says a
    /// request may carry a `ping`, and it also says a request is what describes
    /// the subscription. Sending a ping-only request is safe under the first
    /// reading and unsubscribes everything under the second — and no endpoint is
    /// reachable to find out which. Resending the request already in force is
    /// correct under **both**: the subscription it describes is the one already
    /// in place. It costs a few hundred bytes on a message that arrives every
    /// few seconds.
    ///
    /// A failure to send is not an error here. It means the outbound half is
    /// gone, which the inbound half is about to report on its own — and
    /// answering a keep-alive is not worth a second way of ending a session.
    async fn answer_ping(&mut self) {
        let mut request = self.request.clone();
        request.ping = Some(SubscribeRequestPing { id: 1 });

        if self.outbound.send(request).await.is_err() {
            debug!("could not answer a ping — the outbound stream is gone");
        }
    }

    fn see_slot(&mut self, slot: u64) {
        self.highest_slot = Some(self.highest_slot.map_or(slot, |seen| seen.max(slot)));
    }
}

/// The metric label for a translation failure.
///
/// Mirrors `TransactionProcessor`'s `failure_kind`: a bounded set of labels, so
/// the counter stays a counter and does not become a cardinality problem the
/// day an error message contains a signature.
fn adapter_failure_kind(error: &CoreError) -> &'static str {
    match error {
        CoreError::MissingField { .. } => "missing_field",
        CoreError::ParseError { .. } => "parse_error",
        // `from_grpc` returns only the two above — its doc-comment lists them —
        // but a catch-all label is what keeps a future variant countable instead
        // of unrepresentable.
        _ => "other",
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
