//! Tests for the slot/time pairing: pure state, exercised exhaustively. Whether the bounds are
//! the right numbers is a provider's stream's to say, through the two peaks it reports.

use super::*;
use crate::infra::grpc::test_fixtures::{Snapshot, gauge_for};

/// What the assertions read and nothing in production does, through two `#[cfg(test)]` accessors.
impl<T> SlotTimestampBuffer<T> {
    /// How many payloads are waiting.
    fn pending_payloads(&self) -> usize {
        self.waiting.count()
    }

    /// How many slots are remembered, whatever is known of them.
    fn known_slots(&self) -> usize {
        self.known.len()
    }
}

/// The shape Alchemy sends: no message names a bank. The bank-less tests read through these.
impl SlotTimestampBuffer<Payload> {
    fn on_unbanked_payload(&mut self, slot: u64, payload: Payload) -> Option<Resolved<Payload>> {
        self.on_payload(slot, Bank::Unknown, payload)
    }

    /// The slot's `Finalized` status, then its block-meta: Alchemy's order.
    fn close(&mut self, slot: u64, at: DateTime<Utc>) -> Vec<Resolved<Payload>> {
        let mut released = self.on_finalized(slot, Bank::Unknown);
        released.extend(self.on_block_time(slot, Bank::Unknown, at));
        released
    }

    /// The slot's `Finalized` status, then an empty block-meta.
    fn close_empty(&mut self, slot: u64) -> Vec<Resolved<Payload>> {
        let mut released = self.on_finalized(slot, Bank::Unknown);
        released.extend(self.on_slot_unresolvable(slot, Bank::Unknown));
        released
    }
}

/// A payload standing in for whatever the listener carries; the buffer is generic.
type Payload = u32;

/// Peaks of this test alone: the process-wide ones would carry what other tests reached.
fn fresh_peaks() -> &'static WaitingPeaks {
    Box::leak(Box::new(WaitingPeaks::new()))
}

fn buffer() -> SlotTimestampBuffer<Payload> {
    // Deliberately tiny: three slots say what 256 would, faster.
    SlotTimestampBuffer::with_bounds(3, 5, 2, fresh_peaks())
}

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("a valid instant")
}

// ── the arrival orders, banks aside ─────────────────────────────────

/// Against a release out of arrival order, or before both halves.
#[test]
fn payloads_arriving_before_their_block_time_wait_for_it() {
    let mut buffer = buffer();

    assert!(
        buffer.on_unbanked_payload(10, 1).is_none(),
        "nothing to resolve yet"
    );
    assert!(buffer.on_unbanked_payload(10, 2).is_none());
    assert_eq!(buffer.pending_payloads(), 2);

    let released = buffer.close(10, at(100));

    assert_eq!(
        released,
        vec![
            Resolved {
                payload: 1,
                at: at(100)
            },
            Resolved {
                payload: 2,
                at: at(100)
            },
        ],
        "both payloads, in the order the stream put them in"
    );
    assert_eq!(buffer.pending_payloads(), 0);
}

/// The reverse order, the reason `known` exists: a late payload would wait for a meta already gone.
#[test]
fn a_payload_arriving_after_its_block_time_resolves_at_once() {
    let mut buffer = buffer();

    assert!(buffer.close(10, at(100)).is_empty(), "nothing waiting");

    let resolved = buffer
        .on_unbanked_payload(10, 7)
        .expect("the time is already known");

    assert_eq!(
        resolved,
        Resolved {
            payload: 7,
            at: at(100)
        }
    );
    assert_eq!(buffer.pending_payloads(), 0, "it never had to wait");
}

/// Slots do not interfere: a time for one releases only its own.
#[test]
fn a_block_time_releases_only_its_own_slot() {
    let mut buffer = buffer();

    buffer.on_unbanked_payload(10, 1);
    buffer.on_unbanked_payload(11, 2);

    let released = buffer.close(11, at(200));

    assert_eq!(
        released,
        vec![Resolved {
            payload: 2,
            at: at(200)
        }]
    );
    assert_eq!(buffer.pending_payloads(), 1, "slot 10 is still waiting");
}

// ── banks ───────────────────────────────────────────────────────────

const BANK_A: Bank = Bank::Id(1);
const BANK_B: Bank = Bank::Id(2);

/// Against a key back to the slot alone: bank B's payload would leave as if it were in the chain.
#[test]
fn only_the_finalized_bank_leaves_and_the_other_is_counted_as_losing() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let released = metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer();
        buffer.on_payload(10, BANK_A, 1);
        buffer.on_payload(10, BANK_B, 2);
        buffer.on_block_time(10, BANK_A, at(100));
        buffer.on_block_time(10, BANK_B, at(101));
        let released = buffer.on_finalized(10, BANK_A);
        assert_eq!(
            buffer.pending_payloads(),
            0,
            "bank B's payload is not left waiting"
        );
        released
    });

    assert_eq!(
        released,
        vec![Resolved {
            payload: 1,
            at: at(100)
        }]
    );
    assert_eq!(
        losing_count(&snapshotter.snapshot().into_vec()),
        Some(&DebugValue::Counter(1)),
        "bank B's payload, counted as such"
    );
}

/// Against an instant read from any bank of the slot: bank A's, named first, would be taken.
#[test]
fn a_payload_after_both_halves_leaves_at_once_with_its_banks_instant() {
    for status_first in [true, false] {
        let mut buffer = buffer();
        let mut released = Vec::new();
        if status_first {
            released.extend(buffer.on_finalized(10, BANK_B));
        }
        released.extend(buffer.on_block_time(10, BANK_A, at(101)));
        released.extend(buffer.on_block_time(10, BANK_B, at(102)));
        if !status_first {
            released.extend(buffer.on_finalized(10, BANK_B));
        }
        assert!(released.is_empty(), "nothing was waiting");

        assert_eq!(
            buffer.on_payload(10, BANK_B, 7),
            Some(Resolved {
                payload: 7,
                at: at(102)
            }),
            "status first: {status_first}"
        );
        assert_eq!(buffer.pending_payloads(), 0);
    }
}

/// Against a release on the instant alone, or a second release when the other half comes.
#[test]
fn a_waiting_payload_leaves_once_when_the_second_half_comes() {
    for status_first in [true, false] {
        let mut buffer = buffer();
        assert!(buffer.on_payload(10, BANK_B, 7).is_none());

        let (first, second) = if status_first {
            let first = buffer.on_finalized(10, BANK_B);
            (first, buffer.on_block_time(10, BANK_B, at(102)))
        } else {
            let first = buffer.on_block_time(10, BANK_B, at(102));
            (first, buffer.on_finalized(10, BANK_B))
        };

        assert!(
            first.is_empty(),
            "one half is not enough (status first: {status_first})"
        );
        assert_eq!(
            second,
            vec![Resolved {
                payload: 7,
                at: at(102)
            }]
        );
        assert!(
            buffer.on_finalized(10, BANK_B).is_empty(),
            "a duplicate status releases nothing twice"
        );
    }
}

/// Against a late payload checked only against the instant: it would leave with the kept bank's.
#[test]
fn a_losing_payload_arriving_after_the_verdict_is_dropped_and_counted() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let mut buffer = buffer();
    metrics::with_local_recorder(&recorder, || {
        buffer.on_finalized(10, BANK_A);
        buffer.on_block_time(10, BANK_A, at(100));
        assert!(buffer.on_payload(10, BANK_B, 2).is_none());
    });

    assert_eq!(
        buffer.pending_payloads(),
        0,
        "not left to wait out the window"
    );
    assert_eq!(
        losing_count(&snapshotter.snapshot().into_vec()),
        Some(&DebugValue::Counter(1))
    );
}

/// Against an unknown bank read as a bank of its own: a server that names the kept bank but not
/// the transaction's would see every transaction dropped as losing.
#[test]
fn an_unknown_bank_is_paired_by_slot_with_the_kept_one() {
    let mut buffer = buffer();

    assert!(buffer.on_payload(10, Bank::Unknown, 1).is_none());
    buffer.on_finalized(10, BANK_A);
    let released = buffer.on_block_time(10, BANK_A, at(100));
    assert_eq!(
        released,
        vec![Resolved {
            payload: 1,
            at: at(100)
        }]
    );

    // The other way round: the kept bank's payload, timed by a block-meta naming no bank.
    buffer.on_finalized(11, BANK_A);
    buffer.on_block_time(11, Bank::Unknown, at(110));
    assert_eq!(
        buffer.on_payload(11, BANK_A, 2),
        Some(Resolved {
            payload: 2,
            at: at(110)
        })
    );
}

/// Against a status without a bank read as a bank of its own: every payload would be dropped as losing.
#[test]
fn a_status_naming_no_bank_admits_every_bank() {
    let mut buffer = buffer();

    buffer.on_payload(10, BANK_A, 1);
    buffer.on_payload(10, BANK_B, 2);
    buffer.on_block_time(10, BANK_A, at(100));
    let released = buffer.on_finalized(10, Bank::Unknown);

    assert_eq!(
        released,
        vec![
            Resolved {
                payload: 1,
                at: at(100)
            },
            Resolved {
                payload: 2,
                at: at(100)
            },
        ]
    );
}

/// Against a give-up by slot: the losing bank's empty block-meta would drop the kept bank's payloads.
#[test]
fn an_empty_block_meta_on_the_losing_bank_spares_the_kept_one() {
    let mut buffer = buffer();

    buffer.on_payload(10, BANK_A, 1);
    assert!(buffer.on_slot_unresolvable(10, BANK_B).is_empty());
    buffer.on_finalized(10, BANK_A);

    assert_eq!(
        buffer.on_block_time(10, BANK_A, at(100)),
        vec![Resolved {
            payload: 1,
            at: at(100)
        }]
    );
}

/// Against a release on the block time alone: a slot the cluster has not finalized would leave.
#[test]
fn nothing_leaves_before_the_status_even_with_its_instant() {
    let mut buffer = buffer();

    buffer.on_unbanked_payload(10, 1);
    assert!(buffer.on_block_time(10, Bank::Unknown, at(100)).is_empty());
    assert!(
        buffer.on_unbanked_payload(10, 2).is_none(),
        "a late payload waits too"
    );

    assert_eq!(buffer.pending_payloads(), 2);
    assert_eq!(buffer.on_finalized(10, Bank::Unknown).len(), 2);
}

/// Against a table sized to the window's slot count: the slot at its far edge, still waiting,
/// would lose its verdict to the head's and be evicted instead of released.
#[test]
fn a_slot_at_the_edge_of_the_window_keeps_its_verdict() {
    let mut buffer = SlotTimestampBuffer::<Payload>::new();
    let edge = 1_000;

    buffer.on_unbanked_payload(edge, 1);
    buffer.on_finalized(edge, Bank::Unknown);
    for slot in edge + 1..=edge + MAX_PENDING_SLOTS {
        buffer.close(slot, at(slot as i64));
    }

    assert_eq!(
        buffer.on_block_time(edge, Bank::Unknown, at(1)),
        vec![Resolved {
            payload: 1,
            at: at(1)
        }]
    );
}

/// The losing-bank counter, or `None` when never touched.
fn losing_count(snapshot: &Snapshot) -> Option<&metrics_util::debugging::DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_grpc_losing_bank_transactions_total"
        })
        .map(|(_, _, _, value)| value)
}

// ── the bounds ──────────────────────────────────────────────────────

/// ⚠️ The bound counts slots: a stream that stops delivering evicts nothing, and an outage stays
/// an outage. A time-bounded implementation fails here.
#[test]
fn a_stalled_stream_evicts_nothing() {
    let mut buffer = buffer();

    buffer.on_unbanked_payload(10, 1);
    buffer.on_unbanked_payload(11, 2);
    buffer.on_unbanked_payload(12, 3);

    // Nothing arrived, so nothing is evicted: there is no clock and no method to call.
    assert_eq!(buffer.pending_payloads(), 3);

    // And on reconnect they still resolve.
    assert_eq!(buffer.close(10, at(100)).len(), 1);
    assert_eq!(buffer.close(11, at(101)).len(), 1);
    assert_eq!(buffer.close(12, at(102)).len(), 1);
    assert_eq!(buffer.pending_payloads(), 0);
}

/// Past the slot bound the oldest slot goes; the bound is a distance from the head, not a count.
#[test]
fn a_slot_left_behind_by_the_stream_is_dropped() {
    let mut buffer = buffer(); // window of 3 slots

    buffer.on_unbanked_payload(10, 1);
    buffer.on_unbanked_payload(11, 2);
    buffer.on_unbanked_payload(12, 3);
    buffer.on_unbanked_payload(13, 4);

    assert_eq!(
        buffer.pending_payloads(),
        4,
        "13 − 10 = 3 is still inside the window: holding four slots is not by \
         itself a reason to drop one"
    );

    // The stream moves on, and slot 10 falls out of the window.
    buffer.on_unbanked_payload(14, 5);

    assert!(
        buffer.close(10, at(100)).is_empty(),
        "slot 10 is 4 behind the head and its meta is not coming"
    );
    assert_eq!(buffer.close(11, at(101)).len(), 1);
    assert_eq!(buffer.close(14, at(104)).len(), 1);
}

/// ⚠️ A fork's slot whose meta never comes is never accompanied by 256 others: only a distance
/// from the head ends it, or it stays the resume point for ever.
#[test]
fn an_orphan_slot_does_not_pin_the_buffer_for_ever() {
    let mut buffer = buffer(); // window of 3 slots

    buffer.on_unbanked_payload(10, 1); // its block-meta will never come

    // One or two slots pending at a time, never near a population ceiling.
    for slot in 11..=14 {
        buffer.on_unbanked_payload(slot, slot as Payload);
        buffer.close(slot, at(100 + slot as i64));
    }

    assert_eq!(
        buffer.oldest_pending_slot(),
        None,
        "the orphan was let go when the stream left it behind — otherwise every \
         reconnection would ask to replay from it"
    );
}

/// ⚠️ Bounding slots does not bound memory: here only the payload total is exceeded. And it
/// evicts the newest: the oldest pending slot resolves next, nothing is stale.
#[test]
fn the_payload_bound_evicts_the_newest_slot_not_the_oldest() {
    let mut buffer = buffer(); // 3 slots, 5 payloads

    for payload in 0..4 {
        buffer.on_unbanked_payload(10, payload);
    }
    for payload in 4..6 {
        buffer.on_unbanked_payload(11, payload);
    }

    assert_eq!(
        buffer.pending_payloads(),
        4,
        "slot 11 — the newest, and the one that caused the overflow — is gone; \
         slot 10, whose block-meta is next on the wire, is kept"
    );
    assert_eq!(
        buffer.close(10, at(100)).len(),
        4,
        "the older slot resolves, which is the whole point of keeping it"
    );
    assert!(
        buffer.close(11, at(101)).is_empty(),
        "the burst slot was the one dropped"
    );
}

/// ⚠️ The two bounds evict opposite ends, and swapping them is visible only in which data survives.
#[test]
fn the_two_bounds_evict_opposite_ends() {
    // Slot bound: the oldest goes, its block-meta is not coming.
    let mut by_slots = SlotTimestampBuffer::with_bounds(2, 100, 2, fresh_peaks());
    for slot in 10..=13 {
        by_slots.on_unbanked_payload(slot, slot as Payload);
    }
    assert!(
        by_slots.close(10, at(100)).is_empty(),
        "slot bound: the oldest was evicted"
    );
    assert_eq!(
        by_slots.close(13, at(103)).len(),
        1,
        "slot bound: the newest survived"
    );

    // Payload bound: the newest goes, the oldest is due to resolve.
    let mut by_payloads = SlotTimestampBuffer::with_bounds(100, 2, 2, fresh_peaks());
    for slot in 10..=13 {
        by_payloads.on_unbanked_payload(slot, slot as Payload);
    }
    assert_eq!(
        by_payloads.close(10, at(100)).len(),
        1,
        "payload bound: the oldest survived"
    );
    assert!(
        by_payloads.close(13, at(103)).is_empty(),
        "payload bound: the newest was evicted"
    );
}

/// ⚠️ `known` must be bounded too: every slot adds an entry, waiting or not.
#[test]
fn the_table_of_known_slots_is_bounded_too() {
    let mut buffer = buffer(); // 2 known slots

    for slot in 10..=13 {
        buffer.close(slot, at(100 + slot as i64));
    }

    assert_eq!(buffer.known_slots(), 2, "only the two most recent are kept");
    // The forgotten ones no longer resolve a late payload...
    assert!(
        buffer.on_unbanked_payload(10, 1).is_none(),
        "slot 10's time was forgotten, so its payload must wait"
    );
    // ...while the remembered ones still do.
    assert_eq!(
        buffer.on_unbanked_payload(13, 2),
        Some(Resolved {
            payload: 2,
            at: at(113)
        })
    );
}

/// ⚠️ The eviction has to be counted, or the measurement that replaces the guessed bound reads a
/// silent metric. `Snapshotter::snapshot` is destructive for counters, so it is taken once.
#[test]
fn evicted_payloads_are_counted() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer(); // window of 3 slots
        // Slot 10 holds two payloads; the stream then moves four slots past it.
        buffer.on_unbanked_payload(10, 1);
        buffer.on_unbanked_payload(10, 2);
        for slot in 11..=14 {
            buffer.on_unbanked_payload(slot, slot as Payload);
        }
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        counter_for(&snapshot, "slot_bound"),
        Some(&DebugValue::Counter(2)),
        "both payloads of the evicted slot must be counted, not the slot"
    );
    assert_eq!(
        counter_for(&snapshot, "payload_bound"),
        None,
        "the slot bound is what fired here — a label that never distinguishes \
         is a label that misleads whoever reads this counter"
    );
}

/// ⚠️ The mirror: the payload bound firing while the slot count is legal; unlabelled, the number
/// would be blamed on the wrong bound.
#[test]
fn the_bound_that_evicted_is_recorded_with_the_count() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer(); // 3 slots, 5 payloads
        for payload in 0..4 {
            buffer.on_unbanked_payload(10, payload);
        }
        for payload in 4..6 {
            buffer.on_unbanked_payload(11, payload);
        }
    });

    let snapshot = snapshotter.snapshot().into_vec();

    assert_eq!(
        counter_for(&snapshot, "payload_bound"),
        Some(&DebugValue::Counter(2)),
        "slot 11's two payloads — the newest slot, which this bound evicts"
    );
    assert_eq!(
        counter_for(&snapshot, "slot_bound"),
        None,
        "only two slots were held, so the slot bound never fired"
    );
}

/// ⚠️ Giving up and the slot bound destroy the same payloads; only the label says which happened,
/// and only one is fixed by raising `MAX_PENDING_SLOTS`.
#[test]
fn a_slot_given_up_on_is_counted_under_its_own_reason() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer();
        buffer.on_unbanked_payload(10, 1);
        buffer.on_unbanked_payload(10, 2);
        // The block-meta came, with no instant.
        buffer.close_empty(10);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        counter_for(&snapshot, "unresolvable"),
        Some(&DebugValue::Counter(2)),
        "both payloads, under the reason that says raising the window would \
         change nothing"
    );
    assert_eq!(
        counter_for(&snapshot, "slot_bound"),
        None,
        "no bound was reached — three slots fit"
    );
}

/// Giving up frees the window place it occupied, for slots that will resolve.
#[test]
fn giving_up_on_a_slot_releases_its_place_in_the_window() {
    let mut buffer = buffer(); // 3 slots

    buffer.on_unbanked_payload(10, 1);
    buffer.on_unbanked_payload(11, 2);
    buffer.on_unbanked_payload(12, 3);
    buffer.close_empty(10);

    // A fourth slot now fits without evicting anything.
    buffer.on_unbanked_payload(13, 4);

    assert_eq!(buffer.pending_payloads(), 3);
    assert_eq!(
        buffer.close(11, at(101)).len(),
        1,
        "slot 11 survived — it was not pushed out to make room for 13"
    );
    assert_eq!(buffer.close(13, at(103)).len(), 1);
    assert!(
        buffer.close(10, at(100)).is_empty(),
        "the abandoned slot is gone for good"
    );
}

/// ⚠️ A slot never pending counts nothing, or the metric would count empty blocks.
#[test]
fn giving_up_on_a_slot_with_nothing_waiting_counts_nothing() {
    use metrics_util::debugging::DebuggingRecorder;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer();
        buffer.close_empty(42);
    });

    assert_eq!(
        counter_for(&snapshotter.snapshot().into_vec(), "unresolvable"),
        None,
        "no payload was lost, so nothing may be reported as lost"
    );
}

/// ⚠️ A given-up slot stays given up for a payload not yet arrived, or that payload would leave
/// counted `slot_bound`, the mislabel `Unresolvable` prevents.
#[test]
fn a_payload_arriving_after_its_slot_was_given_up_is_dropped_at_once() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    let mut buffer = buffer();
    metrics::with_local_recorder(&recorder, || {
        // The block-meta came empty before any payload for the slot.
        buffer.close_empty(10);
        assert!(
            buffer.on_unbanked_payload(10, 1).is_none(),
            "there is no instant"
        );
    });

    assert_eq!(
        buffer.pending_payloads(),
        0,
        "it must not be buffered — it would wait out the window and then be \
         blamed on the window"
    );
    assert_eq!(
        counter_for(&snapshotter.snapshot().into_vec(), "unresolvable"),
        Some(&DebugValue::Counter(1)),
        "and the loss is counted under the reason that names its cause"
    );
}

/// ⚠️ A second, empty block-meta must not un-know a found instant; the opposite direction is a
/// real correction.
#[test]
fn giving_up_on_a_slot_never_overwrites_a_time_already_known() {
    let mut buffer = buffer();

    buffer.close(10, at(100));
    buffer.close_empty(10);

    assert_eq!(
        buffer.on_unbanked_payload(10, 1),
        Some(Resolved {
            payload: 1,
            at: at(100)
        }),
        "the instant was known and stays known"
    );
}

/// Where a break should resume from: the oldest slot whose payloads are still waiting.
#[test]
fn the_oldest_pending_slot_is_the_one_that_was_not_finished() {
    let mut buffer = buffer();

    assert_eq!(buffer.oldest_pending_slot(), None, "nothing is waiting");

    buffer.on_unbanked_payload(12, 1);
    buffer.on_unbanked_payload(11, 2);
    assert_eq!(
        buffer.oldest_pending_slot(),
        Some(11),
        "oldest by slot, not by arrival — the replay has to cover both"
    );

    buffer.close(11, at(100));
    assert_eq!(buffer.oldest_pending_slot(), Some(12));
}

/// The counter for one `reason` label, or `None` when never touched.
fn counter_for<'a>(
    snapshot: &'a Snapshot,
    reason: &str,
) -> Option<&'a metrics_util::debugging::DebugValue> {
    snapshot
        .iter()
        .find(|(key, _, _, _)| {
            key.key().name() == "yog_indexer_grpc_untimestamped_transactions_total"
                && key
                    .key()
                    .labels()
                    .any(|l| l.key() == "reason" && l.value() == reason)
        })
        .map(|(_, _, _, value)| value)
}

// ── accounting ──────────────────────────────────────────────────────

/// The running total tracks both paths in and both out: drifting up evicts too early, down never.
#[test]
fn the_pending_count_tracks_every_way_in_and_out() {
    let mut buffer = buffer();

    buffer.on_unbanked_payload(10, 1);
    buffer.on_unbanked_payload(10, 2);
    buffer.on_unbanked_payload(11, 3);
    assert_eq!(buffer.pending_payloads(), 3);

    buffer.close(10, at(100));
    assert_eq!(buffer.pending_payloads(), 1, "released two");

    // A time for a slot never pending must not disturb the count.
    // ⚠️ A near slot: a far one would move the head and evict slot 11 out of the window.
    buffer.close(12, at(200));
    assert_eq!(buffer.pending_payloads(), 1);

    // Resolving straight through `known` never touches pending either.
    buffer.on_unbanked_payload(12, 4);
    assert_eq!(buffer.pending_payloads(), 1);
}

/// ⚠️ The window applies on every path that moves the head: on a quiet stream only block-metas
/// arrive, and a stale slot would survive to become the resume point.
#[test]
fn a_stale_slot_is_evicted_by_block_metas_alone() {
    let mut buffer = buffer(); // window of 3 slots

    buffer.on_unbanked_payload(10, 1); // its block-meta will never come

    // Nothing else is watched; only block-metas arrive.
    for slot in 11..=20 {
        buffer.close(slot, at(100 + slot as i64));
    }

    assert_eq!(
        buffer.oldest_pending_slot(),
        None,
        "the head ran ten slots past a three-slot window — the stale slot must \
         not survive it just because no payload arrived"
    );
    assert_eq!(buffer.pending_payloads(), 0);
}

// ── peaks ───────────────────────────────────────────────────────────

/// ⚠️ Against a gauge that follows the current count: a drained buffer would read 0, and the
/// burst the bound is sized for would be gone by the next scrape.
#[test]
fn the_payload_peak_survives_the_buffer_draining() {
    use metrics_util::debugging::DebuggingRecorder;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer();
        for payload in 1..=4 {
            buffer.on_unbanked_payload(10, payload);
        }
        assert_eq!(buffer.close(10, at(100)).len(), 4);
        assert_eq!(buffer.pending_payloads(), 0);
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        gauge_for(&snapshot, "yog_indexer_grpc_pending_payloads_peak"),
        Some(4.0)
    );
}

/// ⚠️ Against a lag read from the newest slot, or before the head moves: both report 0 here.
#[test]
fn the_slot_lag_peak_is_the_oldest_waiting_slot_behind_the_head() {
    use metrics_util::debugging::DebuggingRecorder;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer(); // window of 3 slots
        buffer.on_unbanked_payload(10, 1);
        buffer.on_unbanked_payload(12, 2);
        buffer.close(10, at(100));
        buffer.close(12, at(102));
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        gauge_for(&snapshot, "yog_indexer_grpc_pending_slot_lag_peak"),
        Some(2.0)
    );
}

/// ⚠️ Against peaks held per buffer: each reconnection builds a new one, which would start the
/// gauges again from its own, lower, numbers.
#[test]
fn every_buffer_reports_to_the_same_peaks() {
    let first = SlotTimestampBuffer::<Payload>::new();
    let second = SlotTimestampBuffer::<Payload>::new();
    assert!(std::ptr::eq(first.waiting.peaks(), second.waiting.peaks()));
}

/// ⚠️ Against a peak taken after the eviction: it would stop at the bound, 5 here, and hide by
/// how much the stream crossed it.
#[test]
fn the_payload_peak_keeps_the_overshoot_the_bound_evicted() {
    use metrics_util::debugging::DebuggingRecorder;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer(); // 5 payloads at most
        for payload in 1..=6 {
            buffer.on_unbanked_payload(10, payload);
        }
        assert_eq!(
            buffer.pending_payloads(),
            0,
            "the slot over the bound was evicted"
        );
    });

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        gauge_for(&snapshot, "yog_indexer_grpc_pending_payloads_peak"),
        Some(6.0)
    );
}

/// ⚠️ Against an export that writes 0: the listener exports once per run, and a run after a
/// raise would hide the peak until the stream beat it again.
#[test]
fn the_peaks_are_exported_as_stored() {
    use metrics_util::debugging::DebuggingRecorder;

    let peaks = fresh_peaks();
    peaks.record(3, 2);

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    metrics::with_local_recorder(&recorder, || peaks.export());

    let snapshot = snapshotter.snapshot().into_vec();
    assert_eq!(
        gauge_for(&snapshot, "yog_indexer_grpc_pending_payloads_peak"),
        Some(3.0)
    );
    assert_eq!(
        gauge_for(&snapshot, "yog_indexer_grpc_pending_slot_lag_peak"),
        Some(2.0)
    );
}
