//! How a span of received slots widens.

use super::*;

use crate::infra::grpc::test_fixtures::at;

/// Mutation: the last slot no longer widened.
#[test]
fn a_later_slot_widens_the_last_slot_with_its_time() {
    let mut span = ReceivedSpan::starting(10, at(0));
    span.include(14, at(2));

    assert_eq!(span.first(), (10, at(0)));
    assert_eq!(span.last(), (14, at(2)));
}

/// Mutation: the first slot taken from the first one to arrive.
#[test]
fn an_earlier_slot_arriving_late_widens_the_first_slot() {
    let mut span = ReceivedSpan::starting(20, at(9));
    span.include(12, at(1));
    span.include(18, at(7));

    assert_eq!(span.first(), (12, at(1)));
    assert_eq!(span.last(), (20, at(9)));
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

/// Mutation: an end equal to the slot taken again, with the time it came with.
#[test]
fn a_slot_on_either_end_changes_nothing() {
    let mut span = ReceivedSpan::starting(10, at(0));
    span.include(14, at(2));
    let before = span;
    span.include(10, at(7));
    span.include(14, at(8));

    assert_eq!(span, before);
}
