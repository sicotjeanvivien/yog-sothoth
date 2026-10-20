use super::*;
use chrono::TimeZone;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, 0).unwrap()
}

/// Frozen through `hour`, and so valued through its last tranche.
fn frozen_through(hour: u32) -> AnalyticProgress {
    AnalyticProgress {
        first_hour: Some(at(0, 0)),
        last_frozen: Some(at(hour, 0)),
        last_tranche: Some(at(hour, 55)),
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
        last_tranche: None,
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
        last_tranche: None,
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

    let left = planned.without_written(frozen_through(12));

    assert!(left.through < left.from);
    assert!(left.tranches_through < left.tranches_from);
    assert_eq!(left.freeze_through, None);
    assert_eq!(left.tvl_hour, None);
}

/// Against the clamp to `through` kept in `plan` only: a cycle built by hand
/// would freeze hours an earlier cycle wrote.
#[test]
fn a_hand_built_cycle_freezes_nothing_past_what_it_rewrites() {
    let hand_built = CycleWrite {
        tranches_from: at(9, 0),
        tranches_through: at(9, 55),
        from: at(9, 0),
        through: at(9, 0),
        freeze_through: Some(at(12, 0)),
        tvl_hour: None,
        at: at(13, 30),
    };

    assert_eq!(
        hand_built
            .without_written(AnalyticProgress::default())
            .freeze_through,
        Some(at(9, 0))
    );
}

/// Against a close set to the tranche's end alone: a swap that lands within the
/// margin would miss a tranche already valued.
#[test]
fn a_tranche_is_valued_ten_minutes_after_its_close_and_not_before() {
    let progress = AnalyticProgress {
        last_tranche: Some(at(12, 0)),
        ..frozen_through(11)
    };

    let early = CycleWrite::plan(at(12, 24), progress);
    assert_eq!(
        (early.tranches_from, early.tranches_through),
        (at(12, 5), at(12, 5))
    );

    let due = CycleWrite::plan(at(12, 25), progress);
    assert_eq!(
        (due.tranches_from, due.tranches_through),
        (at(12, 5), at(12, 10))
    );
}

/// Against the cap dropped, or applied from the newest end.
#[test]
fn an_outage_values_its_tranches_oldest_first_seventy_two_per_cycle() {
    let cycle = CycleWrite::plan(at(20, 30), frozen_through(8));

    assert_eq!(cycle.tranches_from, at(9, 0));
    assert_eq!(cycle.tranches_through, at(14, 55));
}

/// Against the freeze taken from the clock and the hours alone: 10:00 would
/// freeze with the swaps of its tranches from 10:25 on never valued.
#[test]
fn an_hour_waits_for_its_last_tranche_to_freeze() {
    let hand_built = CycleWrite {
        tranches_from: at(9, 0),
        tranches_through: at(10, 20),
        from: at(9, 0),
        through: at(12, 0),
        freeze_through: Some(at(12, 0)),
        tvl_hour: None,
        at: at(13, 30),
    };

    assert_eq!(
        hand_built
            .without_written(AnalyticProgress::default())
            .freeze_through,
        Some(at(9, 0))
    );
}

#[test]
fn the_first_cycle_values_the_closed_tranches_of_the_current_hour() {
    let first = CycleWrite::plan(at(12, 30), AnalyticProgress::default());

    assert_eq!(
        (first.tranches_from, first.tranches_through),
        (at(12, 0), at(12, 15))
    );
}

/// Against the frozen hours left out of the tranches: a cycle behind a
/// concurrent freeze would value again the tranches of an hour nothing reads.
#[test]
fn a_frozen_hour_has_no_tranche_left_to_value() {
    let planned = CycleWrite::plan(
        at(12, 30),
        AnalyticProgress {
            first_hour: Some(at(9, 0)),
            last_frozen: None,
            last_tranche: None,
        },
    );
    assert_eq!(planned.tranches_from, at(9, 0));

    let left = planned.without_written(AnalyticProgress {
        first_hour: Some(at(9, 0)),
        last_frozen: Some(at(10, 0)),
        last_tranche: None,
    });

    assert_eq!(left.tranches_from, at(11, 0));
}

/// Against the tranches left out of `without_written`: a cycle planned before
/// a concurrent one committed would value its tranches a second time.
#[test]
fn a_cycle_loses_the_tranches_valued_since_it_was_planned() {
    let planned = CycleWrite::plan(at(12, 30), frozen_through(10));
    assert_eq!(planned.tranches_from, at(11, 0));

    let left = planned.without_written(AnalyticProgress {
        last_tranche: Some(at(12, 0)),
        ..frozen_through(10)
    });

    assert_eq!(
        (left.tranches_from, left.tranches_through),
        (at(12, 5), at(12, 15))
    );
}
