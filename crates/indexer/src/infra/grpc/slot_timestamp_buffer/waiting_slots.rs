//! The payloads still waiting for their slot's instant, and which of them go
//! when a bound is exceeded.

use std::collections::BTreeMap;

use crate::infra::grpc::metrics::{EvictionReason, GrpcBufferMetrics};

use super::log;

/// One of the two bounds on what waits.
#[derive(Debug, Clone, Copy)]
enum Bound {
    /// The oldest pending slot is too far behind the head.
    Slots,
    /// Too many payloads are waiting.
    Payloads,
}

impl Bound {
    /// The label an eviction under this bound is counted with — recorded,
    /// since an unlabelled count would be read as the wrong ceiling.
    fn reason(self) -> EvictionReason {
        match self {
            Self::Slots => EvictionReason::SlotBound,
            Self::Payloads => EvictionReason::PayloadBound,
        }
    }
}

/// Payloads waiting for their slot's time, in arrival order within a slot, and
/// the two bounds that cap them. Every operation that moves the head applies
/// the bounds itself.
pub(super) struct WaitingSlots<T> {
    pending: BTreeMap<u64, Vec<T>>,
    /// Running total of `pending`'s values, so the payload bound is a
    /// comparison and not a walk.
    count: usize,
    /// The furthest the stream has got, from either half. What the slot bound
    /// measures against — and why no clock is needed: the window advances with
    /// the stream, so a stalled stream evicts nothing.
    head_slot: Option<u64>,
    max_pending_slots: u64,
    max_pending_payloads: usize,
}

impl<T> WaitingSlots<T> {
    pub(super) fn new(max_pending_slots: u64, max_pending_payloads: usize) -> Self {
        Self {
            pending: BTreeMap::new(),
            count: 0,
            head_slot: None,
            max_pending_slots,
            max_pending_payloads,
        }
    }

    /// Hold a payload until its slot's instant is known.
    pub(super) fn push(&mut self, slot: u64, payload: T) {
        self.see(slot);
        self.pending.entry(slot).or_default().push(payload);
        self.count += 1;
        self.enforce_bounds();
    }

    /// Release everything waiting for `slot`, in arrival order: its instant is
    /// known.
    pub(super) fn release(&mut self, slot: u64) -> Vec<T> {
        self.see(slot);
        let released = self.take(slot);
        self.enforce_bounds();
        released
    }

    /// Drop everything waiting for `slot`: its instant will never be known.
    pub(super) fn give_up(&mut self, slot: u64) {
        // An empty block-meta is still the stream telling us where it is.
        self.see(slot);
        self.evict(slot, EvictionReason::Unresolvable);
        self.enforce_bounds();
    }

    /// The oldest slot still waiting, if any — the oldest the session did not
    /// finish, so the one a reconnection resumes from.
    pub(super) fn oldest(&self) -> Option<u64> {
        self.pending.first_key_value().map(|(slot, _)| *slot)
    }

    /// How many payloads are waiting.
    #[cfg(test)]
    pub(super) fn count(&self) -> usize {
        self.count
    }

    /// Drop one slot's payloads, counted and logged under `reason`.
    ///
    /// The single place a pending slot is destroyed, so the counter cannot be
    /// forgotten on one path out of two. Nothing waiting means nothing lost,
    /// and nothing is counted.
    ///
    /// ⚠️ Evicted payloads are **lost**: without an instant they cannot be
    /// written at all. What is a choice is that the loss is counted and logged
    /// — the counter is the only thing that will say a bound was wrong.
    fn evict(&mut self, slot: u64, reason: EvictionReason) {
        let dropped = self.take(slot);
        if dropped.is_empty() {
            return;
        }

        GrpcBufferMetrics::record_evicted(dropped.len(), reason);
        log::evicted(slot, dropped.len(), reason);
    }

    /// Record how far the stream has got.
    fn see(&mut self, slot: u64) {
        self.head_slot = Some(self.head_slot.map_or(slot, |head| head.max(slot)));
    }

    /// Remove and return what waits for `slot`, keeping the count true.
    fn take(&mut self, slot: u64) -> Vec<T> {
        let released = self.pending.remove(&slot).unwrap_or_default();
        self.count -= released.len();
        released
    }

    /// Evict until both bounds hold.
    ///
    /// ⚠️ **Called by every operation that moves the head**, never left to a
    /// caller: on a stream quiet for the watched protocol, block-metas alone
    /// keep the head running, and a stale slot the window did not catch would
    /// be what a reconnection resumes from — hours behind, on a billed replay.
    ///
    /// ⚠️ **Under sustained overload both ends lose**, and no policy fixes
    /// that; the counter says it is happening. What [`Self::victim`] fixes is
    /// the transient burst, which is the case that actually occurs.
    fn enforce_bounds(&mut self) {
        while let Some(bound) = self.exceeded_bound() {
            let Some(slot) = self.victim(bound) else {
                // Unreachable while a bound is exceeded, but a loop that trusts
                // an invariant it does not check is how loops become infinite.
                return;
            };
            self.evict(slot, bound.reason());
        }
    }

    /// Which bound is exceeded, if any.
    fn exceeded_bound(&self) -> Option<Bound> {
        if self.oldest_is_out_of_window() {
            Some(Bound::Slots)
        } else if self.count > self.max_pending_payloads {
            Some(Bound::Payloads)
        } else {
            None
        }
    }

    /// Which slot goes when `bound` is exceeded.
    ///
    /// ⚠️ **The two bounds evict opposite ends**, and getting it backwards is
    /// silent: both branches evict something and count it.
    /// - **Slot bound → the oldest.** It is `max_pending_slots` behind the head,
    ///   so its block-meta is not late, it is not coming.
    /// - **Payload bound → the newest.** Nothing is stale: in steady state one
    ///   or two slots are pending, and the oldest resolves next. Dropping it for
    ///   the burst that caused the overflow destroys the resolvable half.
    ///
    /// ⚠️ **On a replay, oldest-by-slot is not oldest-by-arrival**: older slots
    /// arrive *last*, so a buffer full under the slot bound evicts each replayed
    /// arrival at once. Kept on purpose, and made harmless by ownership: the
    /// buffer belongs to one session and cannot outlive it, so a replay never
    /// arrives into the previous connection's backlog.
    fn victim(&self, bound: Bound) -> Option<u64> {
        let end = match bound {
            Bound::Slots => self.pending.first_key_value(),
            Bound::Payloads => self.pending.last_key_value(),
        };
        end.map(|(slot, _)| *slot)
    }

    /// Whether the oldest pending slot has fallen out of the window — measured
    /// against the head the *stream* has reached, so nothing is evicted while
    /// nothing arrives.
    fn oldest_is_out_of_window(&self) -> bool {
        match (self.oldest(), self.head_slot) {
            (Some(oldest), Some(head)) => head.saturating_sub(oldest) > self.max_pending_slots,
            _ => false,
        }
    }
}
