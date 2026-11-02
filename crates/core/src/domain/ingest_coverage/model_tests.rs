use chrono::{DateTime, TimeZone, Utc};

use super::SlotRange;

fn at(second: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_700_000_000 + second, 0).unwrap()
}

/// Mutation: the high end no longer widened.
#[test]
fn a_higher_slot_widens_the_high_end_with_its_time() {
    let mut range = SlotRange::starting(10, at(0));
    range.include(14, at(2));

    assert_eq!((range.from(), range.from_at()), (10, at(0)));
    assert_eq!((range.to(), range.to_at()), (14, at(2)));
}

/// Mutation: the low end taken from the first slot received, as a replay delivers it.
#[test]
fn a_lower_slot_arriving_late_widens_the_low_end() {
    let mut range = SlotRange::starting(20, at(9));
    range.include(12, at(1));
    range.include(18, at(7));

    assert_eq!((range.from(), range.from_at()), (12, at(1)));
    assert_eq!((range.to(), range.to_at()), (20, at(9)));
}

/// Mutation: an end moved by a slot already inside the range.
#[test]
fn a_slot_inside_the_range_changes_nothing() {
    let mut range = SlotRange::starting(10, at(0));
    range.include(14, at(2));
    let before = range;
    range.include(12, at(5));

    assert_eq!(range, before);
}
