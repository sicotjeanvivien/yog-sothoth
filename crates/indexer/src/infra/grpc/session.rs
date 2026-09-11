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
    SubscribeRequest, SubscribeUpdate, SubscribeUpdateBlockMeta, SubscribeUpdateTransaction,
    subscribe_update::UpdateOneof,
};
use yog_core::{CoreError, domain::Protocol};

use crate::application::source::IngestedTransaction;

use super::{
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
    /// The outbound half of the bidirectional stream.
    ///
    /// ⚠️ **Held, never read, and the underscore says so on purpose.** Nothing
    /// is sent after the subscription — see the note on server pings below.
    /// What keeping this sender alive buys is that the request stream is never
    /// *half-closed*: dropping it ends the outbound direction, which is legal
    /// HTTP/2 and which a server is free to read as the end of the exchange.
    /// Cheaper to hold a sender than to find out which servers do.
    ///
    /// The `_` prefix is this crate's convention for a field whose value is its
    /// liveness rather than its content — `Daemon::_database` is the other one.
    /// It arrived when `infra/grpc`'s blanket `allow(dead_code)` came off and
    /// the build asked, correctly, why a field was never read.
    _outbound: mpsc::Sender<SubscribeRequest>,
    /// The highest slot whose block-meta has arrived — the only slots this
    /// session can claim to have finished. Advanced by block-metas alone: a
    /// transaction update names a slot that is still in flight.
    highest_meta_slot: Option<u64>,
    /// Whether any **data** came off the stream — a transaction or a
    /// block-meta. Not "any message": see [`Self::received_data`].
    received_data: bool,
    /// The cancellation token, so a wait on a full consumer is interruptible.
    shutdown: CancellationToken,
}

impl StreamSession {
    pub(super) fn new(
        downstream: mpsc::Sender<IngestedTransaction>,
        outbound: mpsc::Sender<SubscribeRequest>,
        shutdown: CancellationToken,
    ) -> Self {
        Self {
            buffer: SlotTimestampBuffer::new(),
            downstream,
            _outbound: outbound,
            highest_meta_slot: None,
            received_data: false,
            shutdown,
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
                self.on_block_meta(meta).await
            }
            Some(UpdateOneof::Ping(_)) => {
                // A server ping is **counted and not answered**, and that is a decision.
                //
                // # ⚠️ Why nothing is sent back
                //
                // Because every answer is unsafe under one of the two readings of the
                // proto, and this file cannot tell which is right without a server.
                //
                // A `SubscribeRequest` may carry a `ping`; a `SubscribeRequest` is also
                // what *describes the subscription*. So a ping-only request is a keep-alive
                // under the first reading and an unsubscribe-everything under the second.
                // Resending the whole request avoids that — but then `from_slot` rides
                // along, and under the second reading every ping re-issues the replay, on a
                // message that arrives at a fixed interval. Clearing `from_slot` avoids
                // *that* — and truncates a replay still in flight, since a reconnection
                // rewinds up to `MAX_PENDING_SLOTS` and a ping arrives long before the
                // replay drains. That was this module's answer for a day, under a
                // doc-comment claiming it was "right under both readings"; it was right
                // under one. Found in review, 10 September 2026.
                //
                // Not answering is the only action that is safe under both, and it costs
                // less than it looks:
                //
                // - the connection is kept alive **below** this layer, by HTTP/2 PING
                //   frames — `listener`'s `http2_keep_alive_interval` with
                //   `keep_alive_while_idle`, which is what an idle-timing middlebox
                //   actually watches;
                // - the reference client does the same: `yellowstone-grpc-client` matches
                //   `UpdateOneof::Ping(_)` and yields nothing.
                //
                // The outbound half of the stream is still held open — see the `_outbound`
                // field — because half-closing it is a different question from answering a
                // ping.
                GrpcListenerMetrics::record_update(UpdateKind::Ping);
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
