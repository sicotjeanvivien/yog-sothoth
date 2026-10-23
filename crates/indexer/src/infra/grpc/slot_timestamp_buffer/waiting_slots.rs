//! The payloads still waiting for their slot's instant, and which go when a bound is exceeded.

use std::collections::BTreeMap;

use crate::infra::grpc::metrics::{EvictionReason, GrpcBufferMetrics, WaitingPeaks};

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
    /// The label an eviction under this bound is counted with.
    fn reason(self) -> EvictionReason {
        match self {
            Self::Slots => EvictionReason::SlotBound,
            Self::Payloads => EvictionReason::PayloadBound,
        }
    }
}

/// Payloads waiting for their slot's time, in arrival order within a slot; every operation that
/// moves the head applies the two bounds itself.
pub(super) struct WaitingSlots<T> {
    pending: BTreeMap<u64, Vec<T>>,
    /// Running total of `pending`'s values, so the payload bound is a comparison, not a walk.
    count: usize,
    /// The furthest the stream has got, from either half: the slot bound measures against it, so
    /// a stalled stream evicts nothing.
    head_slot: Option<u64>,
    max_pending_slots: u64,
    max_pending_payloads: usize,
    peaks: &'static WaitingPeaks,
}

impl<T> WaitingSlots<T> {
    pub(super) fn new(
        max_pending_slots: u64,
        max_pending_payloads: usize,
        peaks: &'static WaitingPeaks,
    ) -> Self {
        Self {
            pending: BTreeMap::new(),
            count: 0,
            head_slot: None,
            max_pending_slots,
            max_pending_payloads,
            peaks,
        }
    }

    /// Hold a payload until its slot's instant is known.
    pub(super) fn push(&mut self, slot: u64, payload: T) {
        self.see(slot);
        self.pending.entry(slot).or_default().push(payload);
        self.count += 1;
        self.enforce_bounds();
    }

    /// Release everything waiting for `slot`, in arrival order: its instant is known.
    pub(super) fn release(&mut self, slot: u64) -> Vec<T> {
        self.see(slot);
        let released = self.take(slot);
        self.enforce_bounds();
        released
    }

    /// Take note that the stream reached `slot`, with nothing to release yet.
    pub(super) fn observe(&mut self, slot: u64) {
        self.see(slot);
        self.enforce_bounds();
    }

    /// Drop what waits for `slot` in a bank the cluster did not keep, counted apart from evictions.
    pub(super) fn drop_losing(&mut self, slot: u64, is_losing: impl Fn(&T) -> bool) {
        let Some(waiting) = self.pending.get_mut(&slot) else {
            return;
        };
        let before = waiting.len();
        waiting.retain(|payload| !is_losing(payload));
        let dropped = before - waiting.len();
        if waiting.is_empty() {
            self.pending.remove(&slot);
        }
        if dropped == 0 {
            return;
        }

        self.count -= dropped;
        GrpcBufferMetrics::record_losing_bank(dropped);
        log::losing_bank(slot, dropped);
    }

    /// Drop everything waiting for `slot`: its instant will never be known.
    pub(super) fn give_up(&mut self, slot: u64) {
        // An empty block-meta is still the stream telling us where it is.
        self.see(slot);
        self.evict(slot, EvictionReason::Unresolvable);
        self.enforce_bounds();
    }

    /// The oldest slot still waiting, if any: the one a reconnection resumes from.
    pub(super) fn oldest(&self) -> Option<u64> {
        self.pending.first_key_value().map(|(slot, _)| *slot)
    }

    /// How many payloads are waiting.
    #[cfg(test)]
    pub(super) fn count(&self) -> usize {
        self.count
    }

    /// The peaks this buffer reports to.
    #[cfg(test)]
    pub(super) fn peaks(&self) -> &'static WaitingPeaks {
        self.peaks
    }

    /// Drop one slot's payloads, counted and logged under `reason`; the single place a pending
    /// slot is evicted, so no path forgets the counter. Nothing waiting is not counted.
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
    /// ⚠️ Called by every operation that moves the head, never left to a caller: block-metas alone
    /// move it on a quiet stream, and a stale slot would become the resume point.
    fn enforce_bounds(&mut self) {
        // ⚠️ Before evicting: afterwards a peak never exceeds its bound, and the overshoot is lost.
        self.peaks.record(self.count, self.oldest_lag());
        while let Some(bound) = self.exceeded_bound() {
            let Some(slot) = self.victim(bound) else {
                // Unreachable while a bound is exceeded; checked so the loop cannot spin.
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

    /// Which slot goes when `bound` is exceeded: the oldest under the slot bound (its meta is
    /// not coming), the newest under the payload bound (the oldest resolves next).
    /// ⚠️ Getting the ends backwards is silent: both branches evict something and count it.
    fn victim(&self, bound: Bound) -> Option<u64> {
        let end = match bound {
            Bound::Slots => self.pending.first_key_value(),
            Bound::Payloads => self.pending.last_key_value(),
        };
        end.map(|(slot, _)| *slot)
    }

    /// Whether the oldest pending slot has fallen out of the window.
    fn oldest_is_out_of_window(&self) -> bool {
        self.oldest_lag() > self.max_pending_slots
    }

    /// How far the oldest pending slot trails the head the stream has reached; 0 with nothing waiting.
    fn oldest_lag(&self) -> u64 {
        match (self.oldest(), self.head_slot) {
            (Some(oldest), Some(head)) => head.saturating_sub(oldest),
            _ => 0,
        }
    }
}
