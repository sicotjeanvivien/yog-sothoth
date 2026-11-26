use chrono::{DateTime, TimeZone, Utc};

use super::*;

fn at(hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, minute, 0).unwrap()
}

/// Against the open hour measured: its TVL would be rewritten through the hour, then kept as
/// it stood at the first measure.
#[test]
fn the_tvl_goes_to_the_last_closed_hour() {
    assert_eq!(last_closed_hour(at(20, 35)), at(19, 0));
    assert_eq!(last_closed_hour(at(20, 0)), at(19, 0));
}
