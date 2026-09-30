//! Unit tests for `From<MaterializationBacklogRow> for MaterializationBacklog`.
//!
//! Pure, no DB. The two timestamps share a type, so a swap between them would
//! compile — each test gives them distinct values to make one visible. (The
//! function itself is covered against a real database by
//! `tests/cagg_materialization.rs`.)

use chrono::{DateTime, TimeZone, Utc};
use yog_core::domain::MaterializationBacklog;

use super::MaterializationBacklogRow;

fn at(hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 10, hour, 0, 0).unwrap()
}

#[test]
fn each_column_lands_in_its_field() {
    let backlog = MaterializationBacklog::from(MaterializationBacklogRow {
        aggregate: "meteora_damm_v2_swap_events_hourly".into(),
        watermark: Some(at(12)),
        oldest_pending_at: Some(at(13)),
    });

    assert_eq!(backlog.aggregate, "meteora_damm_v2_swap_events_hourly");
    assert_eq!(backlog.watermark, Some(at(12)));
    assert_eq!(backlog.oldest_pending_at, Some(at(13)));
}

#[test]
fn a_never_materialised_aggregate_keeps_its_pending_row() {
    let backlog = MaterializationBacklog::from(MaterializationBacklogRow {
        aggregate: "meteora_damm_v2_claim_reward_events_hourly".into(),
        watermark: None,
        oldest_pending_at: Some(at(9)),
    });

    assert_eq!(
        (backlog.watermark, backlog.oldest_pending_at),
        (None, Some(at(9)))
    );
}

#[test]
fn nothing_pending_stays_none() {
    let backlog = MaterializationBacklog::from(MaterializationBacklogRow {
        aggregate: "meteora_damm_v2_liquidity_events_hourly".into(),
        watermark: Some(at(12)),
        oldest_pending_at: None,
    });

    assert_eq!(
        (backlog.watermark, backlog.oldest_pending_at),
        (Some(at(12)), None)
    );
}
