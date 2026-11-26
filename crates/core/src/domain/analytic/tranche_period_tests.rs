use chrono::{DateTime, TimeZone, Utc};

use super::*;
use crate::domain::Window;

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
}

fn tranches() -> WrittenTranches {
    WrittenTranches {
        first: at(13, 13),
        through: at(20, 12) + chrono::TimeDelta::minutes(40),
    }
}

/// Against the hour cut at a frozen hour, or its offset added instead of taken back: 1H is the
/// hour through the last tranche written, and the one before it.
#[test]
fn the_last_hour_ends_at_the_last_tranche_written() {
    let period = TranchePeriod::hour_through(tranches(), None);
    assert_eq!(period.as_of, at(20, 12) + chrono::TimeDelta::minutes(40));
    assert_eq!(period.from, at(20, 11) + chrono::TimeDelta::minutes(40));
    assert!(period.is_covered());

    let before = TranchePeriod::hour_through(tranches(), Some(Window::Hour));
    assert_eq!(before.as_of, period.from);
    assert_eq!(before.from, at(20, 10) + chrono::TimeDelta::minutes(40));
}
