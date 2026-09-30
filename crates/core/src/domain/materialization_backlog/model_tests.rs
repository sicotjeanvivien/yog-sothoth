use super::*;
use chrono::TimeZone;

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 15, hour, 0, 0).unwrap()
}

fn pending_since(oldest_pending_at: Option<DateTime<Utc>>) -> MaterializationBacklog {
    MaterializationBacklog {
        aggregate: "meteora_damm_v2_swap_events_hourly".to_string(),
        watermark: Some(at(8)),
        oldest_pending_at,
    }
}

#[test]
fn the_wait_runs_from_the_oldest_pending_row_to_now() {
    let backlog = pending_since(Some(at(9)));
    assert_eq!(backlog.pending_for(at(14)), Some(Duration::hours(5)));
}

/// Whatever else is true of the aggregate — here a watermark hours old, as it
/// is when the indexer has stopped — nothing waiting means nothing late.
#[test]
fn nothing_pending_is_never_late() {
    let backlog = pending_since(None);
    assert_eq!(backlog.pending_for(at(23)), None);
    assert!(backlog.late_by(at(23), Duration::zero()).is_none());
}

#[test]
fn a_row_from_the_future_waits_zero_rather_than_a_negative_time() {
    // Clocks of the database and of the daemon are not the same clock.
    let backlog = pending_since(Some(at(11)));
    assert_eq!(backlog.pending_for(at(10)), Some(Duration::zero()));
}

#[test]
fn late_means_strictly_beyond_the_limit() {
    let backlog = pending_since(Some(at(7)));
    assert!(backlog.late_by(at(11), Duration::hours(4)).is_none());
    assert!(
        backlog
            .late_by(at(11) + Duration::seconds(1), Duration::hours(4))
            .is_some()
    );
}

/// A never-materialised aggregate is judged like any other: by its oldest
/// row. The watermark is `None` there, and must not stand in for "late".
#[test]
fn a_never_materialised_aggregate_is_late_only_once_its_rows_have_waited() {
    let backlog = MaterializationBacklog {
        aggregate: "meteora_damm_v2_claim_reward_events_hourly".to_string(),
        watermark: None,
        oldest_pending_at: Some(at(9)),
    };
    assert!(backlog.late_by(at(11), Duration::hours(4)).is_none());
    assert!(backlog.late_by(at(14), Duration::hours(4)).is_some());
}

#[test]
fn late_by_gives_the_wait_only_past_the_limit() {
    let backlog = pending_since(Some(at(7)));
    assert_eq!(backlog.late_by(at(11), Duration::hours(4)), None);
    assert_eq!(
        backlog.late_by(at(12), Duration::hours(4)),
        Some(Duration::hours(5))
    );
}
