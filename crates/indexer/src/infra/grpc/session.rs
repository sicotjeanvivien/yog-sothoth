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
//! What is dropped with the old session is not lost twice over — **but only
//! because [`StreamSession::resume_from`] is written to make that true.** A
//! payload pending at the moment of the cut is re-delivered by the replay
//! precisely because the resume point is the oldest slot this session did not
//! finish, and *that* is the half the first version got wrong: it resumed one
//! past the highest slot it had seen, which on any mid-block break skipped
//! exactly the pending payloads this paragraph promises are safe. The
//! sentence was true of the design and false of the code, which is the worst
//! of the three possibilities.

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use yellowstone_grpc_proto::prelude::{
    SubscribeRequest, SubscribeRequestPing, SubscribeUpdate, SubscribeUpdateBlockMeta,
    SubscribeUpdateTransaction, subscribe_update::UpdateOneof,
};
use yog_core::{CoreError, domain::Protocol};

use super::{
    ingested_transaction::IngestedTransaction,
    metrics::{DropReason, GrpcListenerMetrics, UpdateKind},
    slot_timestamp_buffer::SlotTimestampBuffer,
    subscription::protocol_of,
    transaction_adapter::from_grpc,
};

/// How many slots a reconnection asks for again, on top of what this session
/// did not finish.
///
/// ⚠️ A **deliberate overlap**, not a margin of error. A block-meta closing slot
/// *N* does not promise that all of *N*'s transactions have arrived — the
/// reverse order is a case the buffer exists to handle — so the closed mark can
/// itself be one short. Re-asking is cheap and exact: the unique key of every
/// event table is `(signature, event_index, timestamp)`, so a row that comes
/// twice is skipped and counted, while a row that never comes leaves a hole
/// nothing will notice. Two is what the reference client uses, for the reason
/// its own comment gives.
const REWIND_SLOTS: u64 = 2;

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
    buffer: SlotTimestampBuffer<PendingTransaction>,
    downstream: mpsc::Sender<IngestedTransaction>,
    /// The outbound half of the bidirectional stream, for answering pings.
    outbound: mpsc::Sender<SubscribeRequest>,
    /// The request this session is subscribed with — resent, unchanged, as the
    /// body of a ping answer. See [`Self::answer_ping`].
    request: SubscribeRequest,
    /// The highest slot whose block-meta has arrived — the only slots this
    /// session can claim to have finished. Advanced by block-metas alone: a
    /// transaction update names a slot that is still in flight.
    highest_meta_slot: Option<u64>,
    /// Whether anything at all came off the stream. Not a slot: a session that
    /// received only a ping delivered nothing, and the difference decides
    /// whether a reconnection counts against the retry budget.
    received_anything: bool,
    /// The cancellation token, so a wait on a full consumer is interruptible.
    shutdown: CancellationToken,
}

impl StreamSession {
    pub(super) fn new(
        downstream: mpsc::Sender<IngestedTransaction>,
        outbound: mpsc::Sender<SubscribeRequest>,
        request: SubscribeRequest,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            buffer: SlotTimestampBuffer::new(),
            downstream,
            outbound,
            request,
            highest_meta_slot: None,
            received_anything: false,
            shutdown,
        }
    }

    /// Whether this session got anything off the stream at all.
    ///
    /// What the listener does with it: a connection that opened and closed
    /// having delivered nothing is a **failing attempt**, not the churn of a
    /// long-lived stream, so it counts against the retry budget. Without this
    /// distinction a server that accepts `subscribe` and closes at once — an
    /// exhausted quota, a token refused at stream level — is retried for ever
    /// at one attempt per second, and the budget never fires.
    pub(super) fn received_anything(&self) -> bool {
        self.received_anything
    }

    /// Where a reconnection should resume from, or `None` when nothing arrived.
    ///
    /// # ⚠️ Not "the highest slot seen", which is what this was and which lost data
    ///
    /// A transaction update names a slot that is **still in flight**: its
    /// block-meta has not arrived, its payloads are sitting in the buffer, and
    /// the buffer dies with this session. Resuming one past that slot therefore
    /// dropped, on every mid-block break, precisely the transactions the break
    /// destroyed — while the module doc above claimed a replay would bring them
    /// back. Found in review, 9 September 2026.
    ///
    /// So the mark is the oldest slot this session did **not finish**: the
    /// oldest one still waiting in the buffer if there is one, and otherwise the
    /// last slot a block-meta closed.
    ///
    /// # ⚠️ And it is rewound, because "closed" is not "complete"
    ///
    /// A block-meta does not promise that its slot's transactions have all
    /// arrived — the reverse order is a case this buffer exists to handle. So
    /// even the closed mark can be one short. [`REWIND_SLOTS`] buys that back at
    /// the only price available, which is duplicates: every event table's unique
    /// key is `(signature, event_index, timestamp)` and a re-inserted row is
    /// skipped and counted. **Overlapping costs a counter; a gap costs rows that
    /// nothing will ever notice are missing.** The reference client makes the
    /// same trade with the same constant, for the reason its own comment gives:
    /// "block_meta can arrive late and events within a slot arrive in random
    /// order".
    pub(super) fn resume_from(&self) -> Option<u64> {
        let unfinished = self
            .buffer
            .oldest_pending_slot()
            .or(self.highest_meta_slot)?;
        Some(unfinished.saturating_sub(REWIND_SLOTS))
    }

    /// Take one update off the stream.
    ///
    /// Every failure below is per-update: counted, logged, stepped over. The
    /// one thing that ends a session is the consumer disappearing.
    pub(super) async fn handle(&mut self, update: SubscribeUpdate) -> SessionState {
        self.received_anything = true;
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

        // A transaction that matched no protocol filter cannot be routed: the
        // pipeline is per-protocol all the way down. Dropped and counted, since
        // it means the request and this reader disagree — and re-deriving the
        // protocol from the account keys here would paper over exactly that.
        let Some(protocol) = protocol else {
            GrpcListenerMetrics::record_dropped(DropReason::Unroutable);
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
        self.see_meta_slot(slot);

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
            match self.emit(resolved.payload, resolved.at).await {
                SessionState::Open => {}
                // Either end of the session: whatever is still in this batch
                // dies with the buffer, and `resume_from` is what asks for it
                // again.
                ended => return ended,
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
                GrpcListenerMetrics::record_dropped(drop_reason(&error));
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
                // The wait is bounded by the shutdown token and by nothing else:
                // back-pressure must survive a slow consumer, not a stop
                // request.
                tokio::select! {
                    sent = self.downstream.send(ingested) => match sent {
                        Ok(()) => {
                            GrpcListenerMetrics::record_emitted();
                            SessionState::Open
                        }
                        Err(_) => SessionState::DownstreamClosed,
                    },
                    _ = self.shutdown.cancelled() => SessionState::ShutdownRequested,
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
        // ⚠️ **Without `from_slot`**, and this is the half the first version
        // missed. Under the reading where a request replaces the subscription,
        // resending one that still carries `from_slot: Some(n)` re-issues the
        // replay on **every** ping — a fixed-interval message — so the same
        // slots would be streamed round and round, over a connection billed by
        // the byte. Dropping it is right under both readings: the subscription
        // in force is already past that point. Found in review, 9 September 2026.
        request.from_slot = None;

        if self.outbound.send(request).await.is_err() {
            debug!("could not answer a ping — the outbound stream is gone");
        }
    }

    /// Record that a block-meta closed a slot.
    ///
    /// Only block-metas move this mark — see [`Self::resume_from`] for why a
    /// transaction's slot is not evidence that the slot is finished.
    fn see_meta_slot(&mut self, slot: u64) {
        self.highest_meta_slot = Some(self.highest_meta_slot.map_or(slot, |seen| seen.max(slot)));
    }
}

/// The reason label for a translation failure.
///
/// Mirrors `TransactionProcessor`'s `failure_kind`: a bounded set of labels, so
/// the counter stays a counter and does not become a cardinality problem the
/// day an error message contains a signature.
fn drop_reason(error: &CoreError) -> DropReason {
    match error {
        CoreError::MissingField { .. } => DropReason::MissingField,
        CoreError::ParseError { .. } => DropReason::ParseError,
        // `from_grpc` returns only the two above — its doc-comment lists them —
        // but a catch-all label is what keeps a future variant countable instead
        // of unrepresentable.
        _ => DropReason::OtherMalformation,
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
