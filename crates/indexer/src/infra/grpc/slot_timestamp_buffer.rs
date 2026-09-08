//! Pairing a Yellowstone stream's two halves: what happened, and when.
//!
//! `SubscribeUpdateTransaction` carries `transaction` and `slot` — and no time.
//! `block_time` lives on `SubscribeUpdateBlockMeta`, a **separate**
//! subscription keyed by slot. Yet `TransactionPosition::timestamp` may not be
//! optional: it is a member of every event table's unique key *and* the
//! TimescaleDB partitioning column, so a transaction without one is not a
//! degraded row, it is an unwritable one.
//!
//! Two unsynchronised streams, then. A transaction of slot *N* can arrive
//! before the block-meta of slot *N*, or after it. This buffer holds whichever
//! came first until the other shows up, and bounds that wait — a block-meta
//! that never arrives must not grow memory without end.
//!
//! It is deliberately **generic over the payload** and knows nothing about
//! protobuf: "given a stream of `(slot, T)` and a stream of `(slot, time)`,
//! yield `(T, time)`". The listener slice instantiates it; the reasoning about
//! time is testable without a wire.
//!
//! # ⚠️ Why the bound counts slots and not seconds
//!
//! This is decision n° 3 of `03 - active/listener-grpc-yellowstone.md`, and the
//! argument is not comfort — it is what each choice does **when things break**.
//!
//! A time bound reads a wall clock, and a wall clock keeps running while the
//! stream is down. A thirty-second outage would then empty this buffer and
//! **destroy transactions whose block-meta was going to arrive on reconnect**:
//! a network fault turned into data loss. A slot bound reads the stream itself.
//! Nothing arrives, so nothing is evicted, and an outage stays an outage.
//!
//! A consequence rather than the argument: with no clock, this module has none
//! to inject and no time to simulate in its tests. (The workspace has no clock
//! abstraction at all — `Utc::now()` is called directly wherever it is needed.)
//!
//! # What the default is, and what it is not
//!
//! [`MAX_PENDING_SLOTS`] is a **ceiling, not an estimate**. The real lag
//! between a block-meta and its slot's transactions is a physical quantity of
//! the provider's stream, and nobody here has measured it — no gRPC endpoint is
//! reachable before the subscription. So the number is picked to be far beyond
//! any plausible lag, precisely so that **the eviction counter is a signal and
//! not background noise**. Reading that counter is how
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` will replace the guess with
//! a measurement, which is what its "borne du tampon posée sur cette mesure"
//! criterion asks for.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use tracing::warn;

use super::metrics::{EvictionReason, GrpcBufferMetrics};

/// How many slots may wait for their block-meta at once.
///
/// 256 slots is ≈ 100 s at Solana's ~400 ms per slot — see the module docs for
/// why this is a ceiling rather than an estimate, and what its counter is for.
pub(crate) const MAX_PENDING_SLOTS: usize = 256;

/// How many payloads may be held across all pending slots.
///
/// ⚠️ Bounding slots does **not** bound memory: 256 slots multiplied by a burst
/// of transactions per slot is an unbounded product. This second limit is what
/// makes the pathological case impossible rather than merely unlikely, and it
/// costs a counter.
pub(crate) const MAX_PENDING_PAYLOADS: usize = 8_192;

/// How many resolved slot times are remembered, for payloads that arrive
/// **after** their block-meta.
///
/// ⚠️ The table nobody thinks to bound. Without it the reverse order loses
/// everything; without a bound on it, memory grows quietly for as long as the
/// process runs.
pub(crate) const MAX_KNOWN_SLOTS: usize = 256;

/// A payload and the block time that was found for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resolved<T> {
    pub(crate) payload: T,
    pub(crate) at: DateTime<Utc>,
}

/// Holds one half of the stream until the other half names its instant.
pub(crate) struct SlotTimestampBuffer<T> {
    /// Payloads waiting for their slot's time, in arrival order within a slot.
    pending: BTreeMap<u64, Vec<T>>,
    /// Running total of `pending`'s values, so the payload bound is a
    /// comparison and not a walk.
    pending_count: usize,
    /// Times already seen, for the payload that arrives after its block-meta.
    known: BTreeMap<u64, DateTime<Utc>>,
    max_pending_slots: usize,
    max_pending_payloads: usize,
    max_known_slots: usize,
}

impl<T> SlotTimestampBuffer<T> {
    pub(crate) fn new() -> Self {
        Self::with_bounds(MAX_PENDING_SLOTS, MAX_PENDING_PAYLOADS, MAX_KNOWN_SLOTS)
    }

    /// Build one with explicit bounds.
    ///
    /// Only the tests pass anything but the constants: reaching 256 slots by
    /// pushing 257 of them would say nothing the three-slot version does not,
    /// and would say it slowly.
    pub(crate) fn with_bounds(
        max_pending_slots: usize,
        max_pending_payloads: usize,
        max_known_slots: usize,
    ) -> Self {
        Self {
            pending: BTreeMap::new(),
            pending_count: 0,
            known: BTreeMap::new(),
            max_pending_slots,
            max_pending_payloads,
            max_known_slots,
        }
    }

    /// Take a payload whose slot is known but whose instant may not be.
    ///
    /// `Some` when the block-meta has already been seen. **`None` means "not
    /// resolved now" and nothing more** — usually buffered, but possibly
    /// dropped on the spot, since inserting can push a bound over and the
    /// payload just handed in can be the one evicted. It is dropped counted and
    /// logged either way, so the caller does the same thing in both cases; what
    /// it must not do is read `None` as "safely waiting". Narrowed after review
    /// on 8 September 2026, where it claimed the stronger thing.
    pub(crate) fn on_payload(&mut self, slot: u64, payload: T) -> Option<Resolved<T>> {
        if let Some(at) = self.known.get(&slot) {
            return Some(Resolved { payload, at: *at });
        }

        self.pending.entry(slot).or_default().push(payload);
        self.pending_count += 1;
        self.enforce_pending_bounds();
        None
    }

    /// Take a slot's instant, releasing everything that was waiting for it.
    ///
    /// The payloads come back **in the order they arrived**, which is the order
    /// the stream put them in.
    ///
    /// # ⚠️ For the caller: `block_time` is optional on the wire
    ///
    /// `SubscribeUpdateBlockMeta::block_time` is an `Option<UnixTimestamp>`, so
    /// a block-meta can arrive carrying no instant at all. It resolves nothing,
    /// and the slot must stay pending — **do not substitute a default, a
    /// receive time, or a neighbouring slot's**. Any of those writes a wrong
    /// value into the partitioning column, where nothing will ever question it.
    ///
    /// This signature takes a `DateTime<Utc>` and not an `Option` precisely so
    /// the decision cannot be deferred to here: a caller with no instant has
    /// nothing to call this with. Slice 1 had to learn the same lesson twice —
    /// "the source did not tell us" is not a value.
    pub(crate) fn on_block_time(&mut self, slot: u64, at: DateTime<Utc>) -> Vec<Resolved<T>> {
        let released = self.pending.remove(&slot).unwrap_or_default();
        self.pending_count -= released.len();

        self.known.insert(slot, at);
        self.enforce_known_bound();

        released
            .into_iter()
            .map(|payload| Resolved { payload, at })
            .collect()
    }

    /// Drop the oldest slots until both pending bounds hold.
    ///
    /// Oldest by slot number, `BTreeMap` ordering them, so "the one least
    /// likely to still be resolved" is `first_key_value`.
    ///
    /// ⚠️ **On a steady stream oldest-by-slot is oldest-by-arrival; on a replay
    /// it is not.** After a reconnect with `from_slot`, older slots arrive
    /// *last*, so a full buffer evicts each new arrival immediately while newer
    /// pending slots survive. Kept deliberately — an older slot really is the
    /// one least likely to still resolve, whenever it turned up — but it means
    /// **a replay into a full buffer loses its own payloads**, silently except
    /// for the counter. Whether to clear this buffer on reconnect is a listener
    /// decision, carried to the ticket rather than guessed at here.
    ///
    /// ⚠️ Evicted payloads are **lost**, and that is not a choice — without an
    /// instant they cannot be written at all. What is a choice is that the loss
    /// is counted and logged rather than silent: the counter is the only thing
    /// that will say the bound was wrong.
    fn enforce_pending_bounds(&mut self) {
        loop {
            // Which bound is binding is recorded, not just that one was: the
            // two cross at 32 payloads per slot, and an unlabelled count would
            // be read as the wrong ceiling. See `EvictionReason`.
            let reason = if self.pending.len() > self.max_pending_slots {
                EvictionReason::SlotBound
            } else if self.pending_count > self.max_pending_payloads {
                EvictionReason::PayloadBound
            } else {
                return;
            };

            let Some((&slot, _)) = self.pending.first_key_value() else {
                // Unreachable while either count is over its bound, but a loop
                // that trusts an invariant it does not check is how loops
                // become infinite.
                return;
            };
            let dropped = self.pending.remove(&slot).unwrap_or_default();
            self.pending_count -= dropped.len();

            GrpcBufferMetrics::record_evicted(dropped.len(), reason);
            warn!(
                slot,
                payloads = dropped.len(),
                bound = reason.as_str(),
                "evicting a slot that never received its block time — its \
                 payloads cannot be timestamped, so they are dropped"
            );
        }
    }

    /// Forget the oldest resolved times once too many are remembered.
    ///
    /// No metric and no log: a forgotten time costs nothing by itself. It only
    /// matters if a payload for that slot turns up afterwards, and *that* loss
    /// is the eviction above — counted there, where it happens.
    ///
    /// ⚠️ Same replay caveat as [`Self::enforce_pending_bounds`], mirrored: a
    /// block time for an older slot, arriving into a full table, is inserted and
    /// evicted in the same call. The payloads already waiting for it are still
    /// released first — `on_block_time` drains before it trims — so what is lost
    /// is only the ability to resolve a *later* arrival for that slot.
    fn enforce_known_bound(&mut self) {
        while self.known.len() > self.max_known_slots {
            let Some((&slot, _)) = self.known.first_key_value() else {
                break;
            };
            self.known.remove(&slot);
        }
    }

    /// How many payloads are waiting. For the tests and for the listener's own
    /// gauge, not for logic.
    pub(crate) fn pending_payloads(&self) -> usize {
        self.pending_count
    }

    /// How many slot times are remembered.
    pub(crate) fn known_slots(&self) -> usize {
        self.known.len()
    }
}

#[cfg(test)]
#[path = "slot_timestamp_buffer_tests.rs"]
mod tests;
