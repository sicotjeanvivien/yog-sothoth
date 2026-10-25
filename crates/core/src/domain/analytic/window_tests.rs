use chrono::{DateTime, TimeZone, Utc};

use super::*;

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
}

fn hours() -> FrozenHours {
    FrozenHours {
        first: at(13, 13),
        last_frozen: at(20, 12),
    }
}

/// Against a period that ends at the last frozen hour instead of its close.
#[test]
fn a_span_ends_at_the_close_of_the_last_frozen_hour() {
    let period = FrozenPeriod::of(hours(), Window::Day, None);
    assert_eq!(period.as_of, at(20, 13));
    assert_eq!(period.from, at(19, 13));
    assert_eq!(period.last_hour(), at(20, 12));
}

/// Against an offset added instead of taken back.
#[test]
fn an_offset_moves_the_span_back_by_its_length() {
    let period = FrozenPeriod::of(hours(), Window::Day, Some(Window::Day));
    assert_eq!(period.as_of, at(19, 13));
    assert_eq!(period.from, at(18, 13));
}

/// Against `>` in place of `>=`: a period starting at the first hour is covered.
#[test]
fn a_span_is_covered_from_the_first_hour_on() {
    let week = FrozenPeriod::of(hours(), Window::Week, None);
    assert_eq!(week.from, at(13, 13));
    assert!(week.is_covered());
    assert!(!FrozenPeriod::of(hours(), Window::Week, Some(Window::Day)).is_covered());
}

/// Against a label that no longer matches what `window=` and migration 029 take.
#[test]
fn the_labels_are_those_of_the_contract() {
    let labels: Vec<_> = Window::ALL.iter().map(|w| w.as_str()).collect();
    assert_eq!(labels, ["24h", "7d", "30d"]);
    assert_eq!(Window::Month.duration(), chrono::TimeDelta::days(30));
}
