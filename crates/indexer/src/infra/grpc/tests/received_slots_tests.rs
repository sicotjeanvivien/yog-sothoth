//! What a session counts as received for good.

use super::*;

use crate::infra::grpc::test_fixtures::{PROTOCOL, at};

/// Past every slot these tests close: nothing waits for a reconnection.
const FAR: Option<u64> = Some(1_000);

fn spans(ranges: &RangesToRecord) -> Vec<(u64, u64)> {
    ranges
        .snapshot()
        .values()
        .map(|range| (range.span.first().0, range.span.last().0))
        .collect()
}

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
