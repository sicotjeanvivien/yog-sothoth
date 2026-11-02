//! What a session counts as received for good, and how its span widens.

use super::*;

use crate::infra::grpc::test_fixtures::{PROTOCOL, at};

/// Past every slot these tests close: nothing waits for a reconnection.
const FAR: Option<u64> = Some(1_000);

fn spans(ranges: &RangesToRecord) -> Vec<(u64, u64)> {
    ranges
        .snapshot()
        .values()
        .map(|range| range.span.slots())
        .collect()
}

// ── the span ────────────────────────────────────────────────────────

/// Mutation: the last slot no longer widened.
#[test]
fn a_later_slot_widens_the_last_slot_with_its_time() {
    let mut span = ReceivedSpan::starting(10, at(0));
    span.include(14, at(2));

    assert_eq!((span.first, span.first_at), (10, at(0)));
    assert_eq!((span.last, span.last_at), (14, at(2)));
}

/// Mutation: the first slot taken from the first one to arrive, as a replay delivers it.
#[test]
fn an_earlier_slot_arriving_late_widens_the_first_slot() {
    let mut span = ReceivedSpan::starting(20, at(9));
    span.include(12, at(1));
    span.include(18, at(7));

    assert_eq!((span.first, span.first_at), (12, at(1)));
    assert_eq!((span.last, span.last_at), (20, at(9)));
}

/// Mutation: an end moved by a slot already inside the span.
#[test]
fn a_slot_inside_the_span_changes_nothing() {
    let mut span = ReceivedSpan::starting(10, at(0));
    span.include(14, at(2));
    let before = span;
    span.include(12, at(5));

    assert_eq!(span, before);
}

// ── what a session received ─────────────────────────────────────────

/// Mutation: the last slot no longer widened by a session's block-metas.
#[test]
fn each_received_block_meta_widens_the_session_span_skipped_slots_included() {
    let ranges = RangesToRecord::default();
    let mut received = ReceivedSlots::new(Arc::from([PROTOCOL]), ranges.clone());

    for slot in [10, 11, 13, 14] {
        received.on_block_meta(slot, at(slot as i64), FAR);
    }

    assert_eq!(spans(&ranges), [(10, 14)]);
}

/// Mutation: the resume point ignored, so a slot a refused replay would lose counts as received.
#[test]
fn a_slot_at_or_past_the_resume_point_waits_until_the_point_passes_it() {
    let ranges = RangesToRecord::default();
    let mut received = ReceivedSlots::new(Arc::from([PROTOCOL]), ranges.clone());

    for slot in 10..=14 {
        received.on_block_meta(slot, at(slot as i64), Some(13));
    }
    assert_eq!(spans(&ranges), [(10, 12)], "13 and 14 would be asked again");

    received.on_block_meta(15, at(15), Some(16));
    assert_eq!(spans(&ranges), [(10, 15)]);
}

/// Mutation: the empty-protocols guard removed, so a pool watched alone claims its protocol.
#[test]
fn a_session_watching_no_whole_protocol_publishes_nothing() {
    let ranges = RangesToRecord::default();
    let mut received = ReceivedSlots::new(Arc::from([]), ranges.clone());

    received.on_block_meta(10, at(10), FAR);

    assert_eq!(spans(&ranges), []);
}

/// Mutation: a session dropped without its span marked ended, so it is never forgotten.
#[test]
fn a_dropped_session_marks_its_span_ended() {
    let ranges = RangesToRecord::default();
    let mut received = ReceivedSlots::new(Arc::from([PROTOCOL]), ranges.clone());
    received.on_block_meta(10, at(10), FAR);
    drop(received);

    let snapshot = ranges.snapshot();
    assert!(snapshot.values().all(|range| range.ended), "{snapshot:?}");
}

/// Mutation: one row for all the protocols watched whole instead of one each.
#[test]
fn a_span_makes_one_row_per_protocol_watched_whole() {
    let range = RangeToRecord {
        protocols: Arc::from([PROTOCOL, Protocol::MeteoraDlmm]),
        span: ReceivedSpan::starting(10, at(10)),
        ended: false,
    };

    let protocols: Vec<Protocol> = range.rows(at(0)).map(|row| row.protocol).collect();
    assert_eq!(protocols, [PROTOCOL, Protocol::MeteoraDlmm]);
}
