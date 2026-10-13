use super::*;
use chrono::TimeZone;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, 0).unwrap()
}

fn frozen_through(hour: u32) -> AnalyticProgress {
    AnalyticProgress {
        first_hour: Some(at(0, 0)),
        last_frozen: Some(at(hour, 0)),
    }
}

/// Against a margin set to zero, or a freeze one hour early.
#[test]
fn an_hour_freezes_ten_minutes_after_its_close_and_not_before() {
    let open = CycleWrite::plan(at(12, 9), frozen_through(10));
    assert_eq!((open.from, open.through), (at(11, 0), at(12, 0)));
    assert_eq!(open.freeze_through, None);

    let freezing = CycleWrite::plan(at(12, 10), frozen_through(10));
    assert_eq!((freezing.from, freezing.through), (at(11, 0), at(12, 0)));
    assert_eq!(freezing.freeze_through, Some(at(11, 0)));
}

#[test]
fn the_first_cycle_starts_at_the_current_hour() {
    let first = CycleWrite::plan(at(12, 30), AnalyticProgress::default());

    assert_eq!((first.from, first.through), (at(12, 0), at(12, 0)));
    assert_eq!(first.freeze_through, None);
}

#[test]
fn hours_written_but_never_frozen_are_rewritten_from_the_first() {
    let progress = AnalyticProgress {
        first_hour: Some(at(9, 0)),
        last_frozen: None,
    };

    let cycle = CycleWrite::plan(at(11, 15), progress);

    assert_eq!((cycle.from, cycle.through), (at(9, 0), at(11, 0)));
    assert_eq!(cycle.freeze_through, Some(at(10, 0)));
}

/// Against a cap dropped (the whole outage in one transaction) or applied from
/// the newest end (the oldest hours skipped).
#[test]
fn an_outage_is_caught_up_oldest_first_a_few_hours_per_cycle() {
    let after_outage = CycleWrite::plan(at(20, 30), frozen_through(8));

    assert_eq!(after_outage.from, at(9, 0));
    assert_eq!(after_outage.through, at(14, 0));
    assert_eq!(after_outage.freeze_through, Some(at(14, 0)));

    let next = CycleWrite::plan(at(20, 35), frozen_through(14));
    assert_eq!((next.from, next.through), (at(15, 0), at(20, 0)));
    assert_eq!(next.freeze_through, Some(at(19, 0)));
}

/// Against `freeze_through` taken from the clock alone: 14:00 was written by
/// an earlier cycle, not this one, and would freeze with its earlier content.
#[test]
fn an_hour_freezes_only_on_a_cycle_that_rewrites_it() {
    let progress = AnalyticProgress {
        first_hour: Some(at(1, 0)),
        last_frozen: None,
    };

    let cycle = CycleWrite::plan(at(15, 30), progress);

    assert_eq!(cycle.through, at(6, 0));
    assert_eq!(cycle.freeze_through, Some(at(6, 0)));
}

#[test]
fn a_clock_behind_the_last_frozen_hour_plans_nothing() {
    let cycle = CycleWrite::plan(at(12, 30), frozen_through(13));

    assert!(cycle.through < cycle.from);
    assert_eq!(cycle.freeze_through, None);
    assert_eq!(cycle.tvl_hour, None);
}

#[test]
fn the_tvl_goes_to_the_hour_of_the_cycle() {
    let cycle = CycleWrite::plan(at(20, 35), frozen_through(8));

    assert_eq!(cycle.tvl_hour, Some(at(20, 0)));
}

/// Against a frozen hour left in the range: a second writer, or a cycle
/// planned before the freeze it now runs after, would write it again.
#[test]
fn a_cycle_loses_the_hours_frozen_since_it_was_planned() {
    let planned = CycleWrite::plan(at(12, 30), frozen_through(10));
    assert_eq!(planned.from, at(11, 0));

    let left = planned.without_frozen(Some(at(12, 0)));

    assert!(left.through < left.from);
    assert_eq!(left.freeze_through, None);
    assert_eq!(left.tvl_hour, None);
}
