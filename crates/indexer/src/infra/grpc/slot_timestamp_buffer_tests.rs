//! Tests for the slot/time pairing.
//!
//! Unlike its neighbour `transaction_adapter_tests`, nothing here is a guess
//! about a wire format: the buffer is pure state, and these tests exercise it
//! exhaustively. What they cannot say is whether the **bound** is the right
//! number — that is a physical property of a provider's stream, measured in
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md`, and the eviction counter
//! is what will say it.

use super::*;

/// A payload standing in for whatever the listener will carry. A `u32` rather
/// than a protobuf message on purpose: the buffer is generic, and a test that
/// needed the real type would be testing the wrong thing.
type Payload = u32;

fn buffer() -> SlotTimestampBuffer<Payload> {
    // Deliberately tiny. Reaching 256 slots by pushing 257 would say nothing
    // three slots do not, and would say it slowly.
    SlotTimestampBuffer::with_bounds(3, 5, 2)
}

fn at(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("a valid instant")
}

// ── the two arrival orders ──────────────────────────────────────────

/// The common order: transactions stream as they execute, the block-meta closes
/// the block. They wait, then come out together.
#[test]
fn payloads_arriving_before_their_block_time_wait_for_it() {
    let mut buffer = buffer();

    assert!(buffer.on_payload(10, 1).is_none(), "nothing to resolve yet");
    assert!(buffer.on_payload(10, 2).is_none());
    assert_eq!(buffer.pending_payloads(), 2);

    let released = buffer.on_block_time(10, at(100));

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

/// The reverse order, which is why `known` exists at all. Without it a payload
/// arriving after its block-meta would wait for a message that already came and
/// be evicted in the end.
#[test]
fn a_payload_arriving_after_its_block_time_resolves_at_once() {
    let mut buffer = buffer();

    assert!(
        buffer.on_block_time(10, at(100)).is_empty(),
        "nothing waiting"
    );

    let resolved = buffer.on_payload(10, 7).expect("the time is already known");

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

    buffer.on_payload(10, 1);
    buffer.on_payload(11, 2);

    let released = buffer.on_block_time(11, at(200));

    assert_eq!(
        released,
        vec![Resolved {
            payload: 2,
            at: at(200)
        }]
    );
    assert_eq!(buffer.pending_payloads(), 1, "slot 10 is still waiting");
}

// ── the bounds ──────────────────────────────────────────────────────

/// ⚠️ **The test that carries decision n° 3.** A stream that stops delivering
/// must cost nothing: the bound counts slots, so no new slot means no eviction,
/// and an outage stays an outage instead of becoming data loss.
///
/// A time-bounded implementation fails here — that is the whole point of
/// writing it down.
#[test]
fn a_stalled_stream_evicts_nothing() {
    let mut buffer = buffer();

    buffer.on_payload(10, 1);
    buffer.on_payload(11, 2);
    buffer.on_payload(12, 3);

    // Time passes in the real world; nothing passes here, because nothing
    // arrived. No tick, no clock, no eviction — there is no method to call.
    assert_eq!(buffer.pending_payloads(), 3);

    // And on reconnect they still resolve.
    assert_eq!(buffer.on_block_time(10, at(100)).len(), 1);
    assert_eq!(buffer.on_block_time(11, at(101)).len(), 1);
    assert_eq!(buffer.on_block_time(12, at(102)).len(), 1);
    assert_eq!(buffer.pending_payloads(), 0);
}

/// Past the slot bound, the oldest slot goes — oldest by slot number, which on
/// this stream is oldest by arrival.
#[test]
fn past_the_slot_bound_the_oldest_slot_is_dropped() {
    let mut buffer = buffer(); // 3 slots

    for slot in 10..=13 {
        buffer.on_payload(slot, slot as Payload);
    }

    assert_eq!(buffer.pending_payloads(), 3, "one slot was evicted");
    // Slot 10 is gone: its time arriving now releases nothing.
    assert!(
        buffer.on_block_time(10, at(100)).is_empty(),
        "the evicted slot must not come back"
    );
    // The three that remain do resolve.
    assert_eq!(buffer.on_block_time(11, at(101)).len(), 1);
    assert_eq!(buffer.on_block_time(12, at(102)).len(), 1);
    assert_eq!(buffer.on_block_time(13, at(103)).len(), 1);
}

/// ⚠️ The second bound, which exists because bounding slots does **not** bound
/// memory. Here the slot count stays legal — 2 of 3 — and only the payload
/// total is exceeded, so this test fails if that limit is dropped.
///
/// **And it evicts the other end.** Block-metas arrive in slot order, so the
/// oldest pending slot is the one due to resolve next; under the payload bound
/// nothing is stale, and dropping the oldest to make room for the burst would
/// destroy the resolvable half. Found in review, 8 September 2026 — this test
/// asserted the opposite until then.
#[test]
fn the_payload_bound_evicts_the_newest_slot_not_the_oldest() {
    let mut buffer = buffer(); // 3 slots, 5 payloads

    for payload in 0..4 {
        buffer.on_payload(10, payload);
    }
    for payload in 4..6 {
        buffer.on_payload(11, payload);
    }

    assert_eq!(
        buffer.pending_payloads(),
        4,
        "slot 11 — the newest, and the one that caused the overflow — is gone; \
         slot 10, whose block-meta is next on the wire, is kept"
    );
    assert_eq!(
        buffer.on_block_time(10, at(100)).len(),
        4,
        "the older slot resolves, which is the whole point of keeping it"
    );
    assert!(
        buffer.on_block_time(11, at(101)).is_empty(),
        "the burst slot was the one dropped"
    );
}

/// ⚠️ **The two bounds evict opposite ends, and nothing else says so.** Both
/// branches drop a slot and count it, so swapping them is invisible except in
/// which data survives. This is the test that fails if they are made alike.
#[test]
fn the_two_bounds_evict_opposite_ends() {
    // Slot bound: the oldest goes, because it is beyond the window and its
    // block-meta is not coming.
    let mut by_slots = SlotTimestampBuffer::with_bounds(2, 100, 2);
    for slot in 10..=13 {
        by_slots.on_payload(slot, slot as Payload);
    }
    assert!(
        by_slots.on_block_time(10, at(100)).is_empty(),
        "slot bound: the oldest was evicted"
    );
    assert_eq!(
        by_slots.on_block_time(13, at(103)).len(),
        1,
        "slot bound: the newest survived"
    );

    // Payload bound: the newest goes, because the oldest is due to resolve.
    let mut by_payloads = SlotTimestampBuffer::with_bounds(100, 2, 2);
    for slot in 10..=13 {
        by_payloads.on_payload(slot, slot as Payload);
    }
    assert_eq!(
        by_payloads.on_block_time(10, at(100)).len(),
        1,
        "payload bound: the oldest survived"
    );
    assert!(
        by_payloads.on_block_time(13, at(103)).is_empty(),
        "payload bound: the newest was evicted"
    );
}

/// ⚠️ The table nobody thinks to bound. Nothing in the pending path touches it,
/// so an unbounded `known` grows for the life of the process without a single
/// other test noticing.
#[test]
fn the_table_of_known_times_is_bounded_too() {
    let mut buffer = buffer(); // 2 known slots

    for slot in 10..=13 {
        buffer.on_block_time(slot, at(100 + slot as i64));
    }

    assert_eq!(buffer.known_slots(), 2, "only the two most recent are kept");
    // The forgotten ones no longer resolve a late payload...
    assert!(
        buffer.on_payload(10, 1).is_none(),
        "slot 10's time was forgotten, so its payload must wait"
    );
    // ...while the remembered ones still do.
    assert_eq!(
        buffer.on_payload(13, 2),
        Some(Resolved {
            payload: 2,
            at: at(113)
        })
    );
}

/// ⚠️ **The eviction has to be counted, not just to happen.** This counter is
/// what `flux-grpc-reel-mesures` reads to replace the guessed bound with a
/// measurement, so a drop that increments nothing would leave that ticket
/// looking at a metric that is silent for the wrong reason.
///
/// Not `#[tokio::test]` and no runtime: `with_local_recorder` installs the
/// recorder on the **current thread** for a closure, and this buffer is
/// synchronous — the same recipe as the persistor test, minus the future.
/// `Snapshotter::snapshot` is destructive for counters, so it is taken once.
#[test]
fn evicted_payloads_are_counted() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer(); // 3 slots
        // Slot 10 holds two payloads; four slots then force it out.
        buffer.on_payload(10, 1);
        buffer.on_payload(10, 2);
        for slot in 11..=13 {
            buffer.on_payload(slot, slot as Payload);
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
         is a label that misleads the ticket reading this counter"
    );
}

/// ⚠️ The mirror, and the one that gives the label its point: the payload bound
/// firing while the slot count is legal. An unlabelled counter reads both of
/// these the same way, and the two ceilings cross at 32 payloads per slot — so
/// above that rate the number would be blamed on the wrong bound.
#[test]
fn the_bound_that_evicted_is_recorded_with_the_count() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer(); // 3 slots, 5 payloads
        for payload in 0..4 {
            buffer.on_payload(10, payload);
        }
        for payload in 4..6 {
            buffer.on_payload(11, payload);
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

/// ⚠️ **The label is the whole reason this method exists.** Giving up on a slot
/// and letting the slot bound expel it destroy the same payloads; only the
/// counter tells the measuring ticket which of the two happened, and only one of
/// the two is fixed by raising `MAX_PENDING_SLOTS`.
#[test]
fn a_slot_given_up_on_is_counted_under_its_own_reason() {
    use metrics_util::debugging::{DebugValue, DebuggingRecorder};

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer();
        buffer.on_payload(10, 1);
        buffer.on_payload(10, 2);
        // The block-meta came, and carried no instant.
        buffer.on_slot_unresolvable(10);
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

/// Giving up frees the window it occupied, which is the other half of the point:
/// a slot nobody can resolve must not spend one of the places meant for slots
/// that will.
#[test]
fn giving_up_on_a_slot_releases_its_place_in_the_window() {
    let mut buffer = buffer(); // 3 slots

    buffer.on_payload(10, 1);
    buffer.on_payload(11, 2);
    buffer.on_payload(12, 3);
    buffer.on_slot_unresolvable(10);

    // A fourth slot now fits without evicting anything.
    buffer.on_payload(13, 4);

    assert_eq!(buffer.pending_payloads(), 3);
    assert_eq!(
        buffer.on_block_time(11, at(101)).len(),
        1,
        "slot 11 survived — it was not pushed out to make room for 13"
    );
    assert_eq!(buffer.on_block_time(13, at(103)).len(), 1);
    assert!(
        buffer.on_block_time(10, at(100)).is_empty(),
        "the abandoned slot is gone for good"
    );
}

/// ⚠️ A slot that was never pending must count **nothing**. The listener calls
/// this on every block-meta with no instant, most of which have no payload
/// waiting; counting those would turn the metric the measuring ticket reads into
/// a count of empty blocks.
#[test]
fn giving_up_on_a_slot_with_nothing_waiting_counts_nothing() {
    use metrics_util::debugging::DebuggingRecorder;

    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();

    metrics::with_local_recorder(&recorder, || {
        let mut buffer = buffer();
        buffer.on_slot_unresolvable(42);
    });

    assert_eq!(
        counter_for(&snapshotter.snapshot().into_vec(), "unresolvable"),
        None,
        "no payload was lost, so nothing may be reported as lost"
    );
}

/// ⚠️ **The test that carries the reconnection decision.** After a `from_slot`
/// replay the old slots arrive last, so a buffer still holding the pre-cut
/// backlog evicts each replayed arrival as it enters. Clearing is what makes the
/// replay able to resolve; this asserts both tables go, since a stale `known`
/// would resolve a replayed payload against a time it no longer has any reason
/// to trust.
#[test]
fn clearing_empties_both_tables() {
    let mut buffer = buffer();

    buffer.on_payload(10, 1);
    buffer.on_block_time(11, at(101));
    assert_eq!(buffer.pending_payloads(), 1);
    assert_eq!(buffer.known_slots(), 1);

    buffer.clear();

    assert_eq!(buffer.pending_payloads(), 0);
    assert_eq!(buffer.known_slots(), 0);
    // And the running total went with it: a count left behind would make the
    // payload bound evict against a number that no longer describes anything.
    assert!(
        buffer.on_payload(11, 2).is_none(),
        "slot 11's time was forgotten with the rest"
    );
    assert_eq!(buffer.pending_payloads(), 1);
}

/// The counter for one `reason` label, or `None` when it was never touched.
fn counter_for<'a>(
    snapshot: &'a [(
        metrics_util::CompositeKey,
        Option<metrics::Unit>,
        Option<metrics::SharedString>,
        metrics_util::debugging::DebugValue,
    )],
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

/// The running total must track both paths in and both paths out, or the
/// payload bound compares against a lie. A count that drifts up evicts too
/// early; one that drifts down never evicts at all.
#[test]
fn the_pending_count_tracks_every_way_in_and_out() {
    let mut buffer = buffer();

    buffer.on_payload(10, 1);
    buffer.on_payload(10, 2);
    buffer.on_payload(11, 3);
    assert_eq!(buffer.pending_payloads(), 3);

    buffer.on_block_time(10, at(100));
    assert_eq!(buffer.pending_payloads(), 1, "released two");

    // A time for a slot that was never pending must not disturb the count.
    buffer.on_block_time(99, at(200));
    assert_eq!(buffer.pending_payloads(), 1);

    // Resolving straight through `known` never touches pending either.
    buffer.on_payload(99, 4);
    assert_eq!(buffer.pending_payloads(), 1);
}
