use super::*;

fn at(minutes_ago: i64, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    Some(now - Duration::minutes(minutes_ago))
}

#[test]
fn recent_event_is_live() {
    let now = Utc::now();
    assert_eq!(
        FreshnessStatus::from_last_event(at(1, now), now),
        FreshnessStatus::Live
    );
}

#[test]
fn mid_age_event_is_delayed() {
    let now = Utc::now();
    assert_eq!(
        FreshnessStatus::from_last_event(at(5, now), now),
        FreshnessStatus::Delayed
    );
}

#[test]
fn old_event_is_stale() {
    let now = Utc::now();
    assert_eq!(
        FreshnessStatus::from_last_event(at(30, now), now),
        FreshnessStatus::Stale
    );
}

#[test]
fn no_event_is_stale() {
    let now = Utc::now();
    assert_eq!(
        FreshnessStatus::from_last_event(None, now),
        FreshnessStatus::Stale
    );
}

#[test]
fn exact_live_boundary_is_live() {
    let now = Utc::now();
    // Exactly 2 minutes old — inclusive lower bound.
    assert_eq!(
        FreshnessStatus::from_last_event(at(2, now), now),
        FreshnessStatus::Live
    );
}
