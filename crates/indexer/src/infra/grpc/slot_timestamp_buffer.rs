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

/// How far behind the stream a slot may still be waiting for its block-meta.
///
/// 256 slots is ≈ 100 s at Solana's ~400 ms per slot — see the module docs for
/// why this is a ceiling rather than an estimate, and what its counter is for.
///
/// ⚠️ **A distance, not a population, and it was written as a population.**
/// Until 9 September 2026 this bound fired on "257 slots pending at once",
/// which in steady state — one or two pending — never happens. A single slot
/// whose block-meta never came (a fork, at `confirmed`) therefore sat in the
/// map for the life of the session: it was never evicted, so it stayed the
/// oldest pending slot for ever, and `session::StreamSession::resume_from`
/// answered with it at every reconnection — asking a bandwidth-billed provider
/// to replay from a slot hours behind, or burning the retry budget on a request
/// past its retention. Found in review, 9 September 2026. The docs already
/// described a distance ("≈ 100 s"); the code now agrees with them.
pub(crate) const MAX_PENDING_SLOTS: u64 = 256;

/// How many payloads may be held across all pending slots.
///
/// ⚠️ Bounding slots does **not** bound memory: 256 slots multiplied by a burst
/// of transactions per slot is an unbounded product. This second limit closes
/// that, and costs a counter.
///
/// ⚠️ But it bounds a **count, not bytes**, and the ceiling it sets is not
/// small. `T` will be a whole `SubscribeUpdateTransactionInfo` — meta, inner
/// instructions, log messages, pre/post balances — on the order of 5–20 KB for
/// a Meteora swap, so 8 192 of them is roughly **40–160 MB resident in this
/// buffer alone**. That is bounded, which is the point, but it is a number
/// slice 3 has to know when it sizes the process. Raised in review,
/// 8 September 2026, where this doc claimed to make the pathological case
/// "impossible" without saying at what price.
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
    /// What is already settled about a slot, for the payload that arrives
    /// **after** its block-meta: `Some(at)` when the meta named an instant,
    /// `None` when it came empty and the slot was given up.
    ///
    /// ⚠️ Holding the dead slots in the *same* table is what keeps the reverse
    /// order honest in both directions. Found in review on 9 September 2026:
    /// with only the resolved times here, a transaction arriving after its
    /// slot's empty block-meta was buffered again — it took a place in the
    /// window and left counted `slot_bound`, which is exactly the mislabel
    /// [`EvictionReason::Unresolvable`] was added to prevent. One table also
    /// means one bound and one eviction rule, rather than a third of each.
    known: BTreeMap<u64, Option<DateTime<Utc>>>,
    /// The furthest the stream has got, from either half. What the slot bound
    /// measures against — and the reason this buffer still needs no clock: the
    /// window advances with the stream, so a stalled stream evicts nothing.
    head_slot: Option<u64>,
    max_pending_slots: u64,
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
        max_pending_slots: u64,
        max_pending_payloads: usize,
        max_known_slots: usize,
    ) -> Self {
        Self {
            pending: BTreeMap::new(),
            pending_count: 0,
            known: BTreeMap::new(),
            head_slot: None,
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
        self.see(slot);

        match self.known.get(&slot) {
            Some(Some(at)) => return Some(Resolved { payload, at: *at }),
            // The slot's one chance to be named came and went empty. Waiting
            // for a second block-meta that will not come would spend a place in
            // the window and end in an eviction blamed on the window's size.
            Some(None) => {
                GrpcBufferMetrics::record_evicted(1, EvictionReason::Unresolvable);
                warn!(
                    slot,
                    "a payload arrived for a slot already given up — its block \
                     time will never come, so it is dropped"
                );
                return None;
            }
            None => {}
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
    /// a block-meta can arrive carrying no instant at all. It resolves nothing —
    /// **do not substitute a default, a receive time, or a neighbouring
    /// slot's**. Any of those writes a wrong value into the partitioning
    /// column, where nothing will ever question it.
    ///
    /// This signature takes a `DateTime<Utc>` and not an `Option` precisely so
    /// the decision cannot be deferred to here: a caller with no instant has
    /// nothing to call this with. Slice 1 had to learn the same lesson twice —
    /// "the source did not tell us" is not a value.
    ///
    /// What such a caller has instead is [`Self::on_slot_unresolvable`]: the
    /// slot's one chance to be named has come and gone empty, so leaving it
    /// pending would only spend the window on it and mislabel its exit.
    pub(crate) fn on_block_time(&mut self, slot: u64, at: DateTime<Utc>) -> Vec<Resolved<T>> {
        self.see(slot);

        let released = self.pending.remove(&slot).unwrap_or_default();
        self.pending_count -= released.len();

        self.known.insert(slot, Some(at));
        self.enforce_known_bound();
        // ⚠️ **The window advances here too, so it has to be applied here too.**
        // Until 10 September 2026 this was called from `on_payload` alone, and
        // the hole it left is the exact case the window was introduced for: a
        // slot whose meta never comes, on a stream that goes quiet. Block-metas
        // keep arriving every ~400 ms and push the head thousands of slots
        // ahead, but with no payload to trigger enforcement the stale slot
        // stays — and `session::StreamSession::resume_from` then answers with
        // it, asking a bandwidth-billed provider to replay hours.
        self.enforce_pending_bounds();

        released
            .into_iter()
            .map(|payload| Resolved { payload, at })
            .collect()
    }

    /// Give up on a slot: nothing will ever name its instant.
    ///
    /// # ⚠️ Why this exists rather than letting the bound handle it
    ///
    /// Both roads end in the same eviction, so the difference is only what the
    /// counter says — which is the whole difference, since that counter is the
    /// one `02 - backlog/pre-v02/flux-grpc-reel-mesures.md` reads to size the
    /// window. A slot nobody can resolve, left to the slot bound, waits out the
    /// full window and leaves labelled `slot_bound`; the ticket reads "the
    /// window is too small", raises `MAX_PENDING_SLOTS`, and the number does not
    /// move. Worse, while it waits it occupies one of the window's places
    /// against slots that *would* have resolved.
    ///
    /// Two callers on the listener's side, both meaning "this slot has no
    /// instant to give":
    ///
    /// - a `SubscribeUpdateBlockMeta` arrived for the slot and its `block_time`
    ///   was `None` — the case [`Self::on_block_time`] refuses to take, because
    ///   a signature that accepted an `Option` would invite a default;
    /// - a slot the listener knows was abandoned (a fork), if it ever learns so.
    ///
    /// Calling it for a slot that is not pending is a no-op, and counts nothing.
    pub(crate) fn on_slot_unresolvable(&mut self, slot: u64) {
        // An empty block-meta is still the stream telling us where it is.
        self.see(slot);
        self.evict(slot, EvictionReason::Unresolvable);
        // Remembered as dead, not merely emptied: a payload for this slot can
        // still arrive — that is the whole reason `known` exists — and it must
        // meet the answer straight away rather than wait out the window.
        //
        // ⚠️ But never *over* an instant already found. A second block-meta for
        // the same slot can arrive carrying nothing — a duplicate, a
        // re-emission after a fork — and letting it overwrite `Some(at)` would
        // throw away a timestamp that was known, then drop every late payload
        // for the slot as unresolvable while its instant sat one branch away.
        // Found in review, 9 September 2026; the opposite direction
        // (`None` → `Some`) is fine and is what a real correction looks like.
        self.known.entry(slot).or_insert(None);
        self.enforce_known_bound();
        self.enforce_pending_bounds();
    }

    /// Drop the oldest slots until both pending bounds hold.
    ///
    /// # ⚠️ Which slot goes depends on which bound fired, and the two are opposite
    ///
    /// Both evictions drop "the slot least likely to still be resolved", but
    /// that is a **different slot** in each case, and treating them alike loses
    /// exactly the data that was about to be saved.
    ///
    /// Block-metas arrive in slot order, so the *oldest* pending slot is the one
    /// whose meta is next on the wire.
    ///
    /// - **Slot bound** → drop the **oldest**. It is `max_pending_slots` behind
    ///   the head, so its meta is not merely late, it is not coming.
    /// - **Payload bound** → drop the **newest**. Nothing here is stale: in
    ///   steady state one or two slots are pending, and the oldest is due to
    ///   resolve next. Dropping it to make room for the burst that caused the
    ///   overflow destroys the resolvable half. Found in review, 8 September
    ///   2026: a one-payload overage was destroying a whole older slot —
    ///   200 payloads whose block-meta was the next message — while the burst
    ///   survived.
    ///
    /// ⚠️ **Under sustained overload both ends lose**, and no policy fixes that;
    /// the counter is what says it is happening. What this rule fixes is the
    /// transient burst, which is the case that actually occurs.
    ///
    /// ⚠️ **On a steady stream oldest-by-slot is oldest-by-arrival; on a replay
    /// it is not.** After a reconnect with `from_slot`, older slots arrive
    /// *last*, so a full buffer under the slot bound evicts each new arrival
    /// immediately. Kept deliberately — an older slot really is the one least
    /// likely to resolve, whenever it turned up — but it means **a replay into a
    /// full buffer loses its own payloads**, silently except for the counter.
    /// That is decided, and decided by ownership rather than by a rule this
    /// buffer would have to follow: a buffer belongs to one subscription
    /// (`session::StreamSession`) and cannot outlive it, so a replay never
    /// arrives into the previous connection's backlog.
    ///
    /// ⚠️ Evicted payloads are **lost**, and that is not a choice — without an
    /// instant they cannot be written at all. What is a choice is that the loss
    /// is counted and logged rather than silent: the counter is the only thing
    /// that will say the bound was wrong.
    fn enforce_pending_bounds(&mut self) {
        loop {
            // Which bound is binding is recorded, not just that one was: an
            // unlabelled count would be read as the wrong ceiling, and the two
            // answer different questions. See `EvictionReason`.
            let reason = if self.oldest_is_out_of_window() {
                EvictionReason::SlotBound
            } else if self.pending_count > self.max_pending_payloads {
                EvictionReason::PayloadBound
            } else {
                return;
            };

            // The end depends on the bound — see this function's docs. Getting
            // this backwards is silent: both branches evict something, both
            // count it, and only the data tells them apart.
            let victim = match reason {
                EvictionReason::SlotBound => self.pending.first_key_value(),
                EvictionReason::PayloadBound => self.pending.last_key_value(),
                // Not a bound: it is a fact about one named slot, and it comes
                // in through `on_slot_unresolvable`. Reaching it here would
                // mean a bound was raised without an end to evict from.
                EvictionReason::Unresolvable => None,
            };
            let Some((&slot, _)) = victim else {
                // Unreachable while either count is over its bound, but a loop
                // that trusts an invariant it does not check is how loops
                // become infinite.
                return;
            };
            self.evict(slot, reason);
        }
    }

    /// Whether the oldest pending slot has fallen out of the window.
    ///
    /// The comparison is against the head *the stream* has reached, so no time
    /// passes here on its own — decision n° 3 of the ticket, unchanged: nothing
    /// arrives, nothing is evicted.
    fn oldest_is_out_of_window(&self) -> bool {
        match (self.pending.first_key_value(), self.head_slot) {
            (Some((&oldest, _)), Some(head)) => {
                head.saturating_sub(oldest) > self.max_pending_slots
            }
            _ => false,
        }
    }

    /// Record how far the stream has got.
    fn see(&mut self, slot: u64) {
        self.head_slot = Some(self.head_slot.map_or(slot, |head| head.max(slot)));
    }

    /// Drop one named slot's payloads, counted and logged under `reason`.
    ///
    /// The single place a pending slot is destroyed, so the counter cannot be
    /// forgotten on one path out of two — which is the shape of defect the
    /// eviction rule above has already produced once.
    fn evict(&mut self, slot: u64, reason: EvictionReason) {
        let dropped = self.pending.remove(&slot).unwrap_or_default();
        if dropped.is_empty() {
            // Nothing was waiting: there is no loss to count, and counting a
            // zero would make the metric say a slot was destroyed.
            return;
        }
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

    /// The oldest slot still waiting for an instant, if any.
    ///
    /// For the caller that has to say **where to resume** after a break: these
    /// payloads die with the buffer, so this is the oldest slot the connection
    /// did not finish. See `session::StreamSession::resume_from`.
    pub(crate) fn oldest_pending_slot(&self) -> Option<u64> {
        self.pending.first_key_value().map(|(slot, _)| *slot)
    }
}

#[cfg(test)]
#[path = "slot_timestamp_buffer_tests.rs"]
mod tests;
