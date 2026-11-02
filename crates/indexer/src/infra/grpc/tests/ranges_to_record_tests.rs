//! The hand-over: one entry per session, and the rows a span makes.

use super::*;

use crate::infra::grpc::test_fixtures::{PROTOCOL, at};

fn range(first: u64, last: u64) -> RangeToRecord {
    let mut span = ReceivedSpan::starting(first, at(first as i64));
    span.include(last, at(last as i64));
    RangeToRecord {
        protocols: Arc::from([PROTOCOL]),
        span,
        ended: false,
    }
}

/// Mutation: one entry for every session, so a reconnection overwrites the last span before.
#[test]
fn each_session_keeps_its_own_entry() {
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20));
    ranges.publish(at(1), range(30, 40));

    assert_eq!(ranges.snapshot().len(), 2);
}

/// Mutation: `end` not marking the session's entry.
#[test]
fn an_ended_session_is_marked() {
    let ranges = RangesToRecord::default();
    ranges.publish(at(0), range(10, 20));
    ranges.end(at(0));

    assert!(ranges.snapshot()[&at(0)].ended);
}

/// Mutation: one row for all the protocols watched whole instead of one each.
#[test]
fn a_span_makes_one_row_per_protocol_watched_whole() {
    let mut range = range(10, 20);
    range.protocols = Arc::from([PROTOCOL, Protocol::MeteoraDlmm]);

    let rows: Vec<(Protocol, u64, u64)> = range
        .rows(at(0))
        .map(|row| (row.protocol, row.first_slot, row.last_slot))
        .collect();
    assert_eq!(rows, [(PROTOCOL, 10, 20), (Protocol::MeteoraDlmm, 10, 20)]);
}
