//! Unit tests for the step of a series.

use chrono::TimeDelta;
use yog_core::domain::Window;

use super::{BucketPeriod, SeriesInterval};

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
