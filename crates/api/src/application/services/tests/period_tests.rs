//! Unit tests for the step of a series.

use chrono::TimeDelta;
use yog_core::domain::Window;

use crate::testing::{at_october, make_frozen_hours};
use yog_core::domain::{FrozenPeriod, TranchePeriod, WrittenTranches};

use super::{Availability, BucketPeriod, DrawnPeriod, ReadPeriod, SeriesInterval, measured_from};

/// Mutation: a window given another step than the one its bars are drawn at, or a step that does
/// not fit it.
#[test]
fn each_window_has_its_own_step_and_the_step_fits_it() {
    for (window, step) in [
        (Window::Hour, SeriesInterval::FiveMinutes),
        (Window::Day, SeriesInterval::Hour),
        (Window::Week, SeriesInterval::SixHours),
        (Window::Month, SeriesInterval::Day),
    ] {
        assert_eq!(SeriesInterval::of(window), step);
        assert!(step.fits(window), "{window:?}");
    }
}

/// Mutation: 6 hours or 5 minutes taken off their window, or 1h and 1d taken over the last hour.
#[test]
fn a_step_is_refused_off_the_windows_it_cuts() {
    use SeriesInterval::{Day, FiveMinutes, Hour, SixHours};
    for (step, window) in [
        (FiveMinutes, Window::Day),
        (SixHours, Window::Hour),
        (SixHours, Window::Day),
        (SixHours, Window::Month),
        (Hour, Window::Hour),
        (Day, Window::Hour),
    ] {
        assert!(!step.fits(window), "{step:?} {window:?}");
    }
    assert!(Hour.fits(Window::Week) && Day.fits(Window::Week));
}

/// Mutation: a bucket read at another width than its name, every 6-hour bar a day wide.
#[test]
fn a_bucket_has_its_width() {
    assert_eq!(BucketPeriod::Hour.width(), TimeDelta::hours(1));
    assert_eq!(BucketPeriod::SixHours.width(), TimeDelta::hours(6));
    assert_eq!(BucketPeriod::Day.width(), TimeDelta::days(1));
}

/// `make_frozen_hours`: covered from the 13th at 13:00, 7 days whole at the 20th at 13:00.
fn frozen(window: Window) -> FrozenPeriod {
    FrozenPeriod::of(make_frozen_hours(), window, None)
}

/// Mutation: `>=` turned `>` — a window covered to the hour stays closed — or the window's length
/// left out of when it opens.
#[test]
fn a_window_opens_the_hour_the_facts_cover_it_whole() {
    let now = at_october(20, 12);

    let short = FrozenPeriod {
        as_of: at_october(20, 12),
        ..frozen(Window::Week)
    };
    let week = Availability::of(Window::Week, None, Some(short), now);
    assert!(!week.open);
    assert_eq!(week.opens_at, Some(at_october(20, 13)));

    let week = Availability::of(Window::Week, None, Some(frozen(Window::Week)), now);
    assert!(week.open);
    assert_eq!(week.opens_at, None);
}

/// Mutation: the month judged on its coverage alone — 30 days read by the dashboard on a guess.
#[test]
fn the_month_is_closed_with_no_date_however_covered() {
    let whole = FrozenPeriod {
        covered_since: at_october(1, 0),
        ..frozen(Window::Month)
    };

    let month = Availability::of(Window::Month, None, Some(whole), at_october(20, 12));

    assert!(!month.open);
    assert_eq!(month.opens_at, None);
}

/// Mutation: a window before any fact read as open, or as late.
#[test]
fn before_any_fact_a_window_is_closed_and_nothing_is_late() {
    let none = Availability::of::<FrozenPeriod>(Window::Day, None, None, at_october(20, 12));

    assert!(!none.open);
    assert_eq!(none.opens_at, None);
    assert!(!none.late);
}

/// Mutation: one threshold for both — a frozen hour late after 30 minutes, or a tranche after 2 h;
/// `>` turned `>=`, on time at the threshold read as late.
#[test]
fn a_frozen_hour_runs_late_after_two_hours_and_a_tranche_after_thirty_minutes() {
    let as_of = at_october(20, 12);
    let hour = FrozenPeriod {
        as_of,
        ..frozen(Window::Day)
    };
    let tranche = TranchePeriod::hour_through(
        WrittenTranches {
            first: at_october(13, 13),
            through: as_of,
        },
        None,
    );
    let late = |period: ReadPeriod, past: TimeDelta| {
        Availability::of(Window::Day, None, Some(period), as_of + past).late
    };

    assert!(!late(ReadPeriod::Frozen(hour), TimeDelta::hours(2)));
    assert!(late(ReadPeriod::Frozen(hour), TimeDelta::minutes(121)));
    assert!(!late(ReadPeriod::Tranche(tranche), TimeDelta::minutes(30)));
    assert!(late(ReadPeriod::Tranche(tranche), TimeDelta::minutes(31)));
}

/// Mutation: a drawn period judged on its own coverage — a block a measure covers in part closed.
#[test]
fn a_drawn_period_opens_on_the_facts_not_on_its_measure() {
    let measured = FrozenPeriod {
        covered_since: at_october(20, 0),
        ..frozen(Window::Day)
    };
    let drawn = DrawnPeriod {
        period: ReadPeriod::Frozen(measured),
        facts_since: at_october(13, 13),
    };

    assert!(Availability::of(Window::Day, None, Some(drawn), at_october(20, 12)).open);
    assert!(!Availability::of(Window::Day, None, Some(measured), at_october(20, 12)).open);
}

/// Mutation: an `offset` read judged on its moved end — late on every healthy read, its `opensAt`
/// a window too soon.
#[test]
fn an_offset_read_is_judged_at_the_end_it_was_moved_from() {
    let hours = make_frozen_hours();
    let moved = FrozenPeriod::of(hours, Window::Day, Some(Window::Day));
    let on_time = at_october(20, 13) + TimeDelta::hours(1);

    let previous = Availability::of(Window::Day, Some(Window::Day), Some(moved), on_time);
    assert!(previous.open);
    assert!(!previous.late);

    let not_yet = FrozenPeriod {
        covered_since: at_october(19, 0),
        ..moved
    };
    let previous = Availability::of(Window::Day, Some(Window::Day), Some(not_yet), on_time);
    assert!(!previous.open);
    assert_eq!(previous.opens_at, Some(at_october(21, 0)));
}

/// Mutation: a measure not yet measured given `asOf + window`, a date that never arrives.
#[test]
fn a_measure_not_yet_measured_has_no_date() {
    let unmeasured = measured_from(frozen(Window::Day), None);

    let day = Availability::of(Window::Day, None, Some(unmeasured), at_october(20, 13));

    assert!(!day.open);
    assert_eq!(day.opens_at, None);
}
