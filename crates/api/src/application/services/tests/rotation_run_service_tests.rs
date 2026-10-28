//! Unit tests for `RotationRunService`: the coverage of the lists, the totals and the bars, and
//! the end a later page keeps.

use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{Protocol, RotationRunCursor, RotationRunOrder, RotationRunScope, Window};

use super::{RotationRunService, RotationRunsError};
use crate::application::BucketPeriod;
use crate::testing::{MockRotationRunRepo, at_october, frozen_hours_repo, make_frozen_hours};

fn service(repo: &Arc<MockRotationRunRepo>) -> RotationRunService {
    RotationRunService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone())
}

fn minutes(at: DateTime<Utc>, minutes: i64) -> DateTime<Utc> {
    at + TimeDelta::minutes(minutes)
}

/// Mutation: the coverage dropped, taken from the facts, or without the gap — a window the runs
/// start inside summed as whole, an unmeasured protocol covered, or a run cut by it counted.
#[tokio::test]
async fn the_totals_are_covered_from_the_first_measured_tranche() {
    let measured = Arc::new(MockRotationRunRepo {
        written_through: Some(minutes(at_october(20, 13), 20)),
        measured_since: Some(minutes(at_october(19, 10), 5)),
        ..MockRotationRunRepo::default()
    });
    let unmeasured = Arc::new(MockRotationRunRepo {
        written_through: Some(minutes(at_october(20, 13), 20)),
        ..MockRotationRunRepo::default()
    });
    let protocol = Protocol::MeteoraDammV2;

    let day = service(&measured)
        .get_totals(Window::Day, protocol)
        .await
        .unwrap();
    let week = service(&measured)
        .get_totals(Window::Week, protocol)
        .await
        .unwrap();
    let not_yet = service(&unmeasured)
        .get_totals(Window::Day, protocol)
        .await
        .unwrap();

    let day_period = day.period.unwrap();
    assert_eq!(day_period.as_of, minutes(at_october(20, 13), 20));
    assert_eq!(day_period.from, minutes(at_october(19, 13), 20));
    assert_eq!(day.items.len(), 1);
    assert!(week.items.is_empty());
    assert_eq!(
        week.period.unwrap().covered_since,
        minutes(at_october(19, 10), 35)
    );
    assert!(not_yet.items.is_empty());
}

fn cursor_ending(as_of: DateTime<Utc>) -> RotationRunCursor {
    RotationRunCursor {
        scope: RotationRunScope::Protocol(Protocol::MeteoraDammV2),
        window: Window::Day,
        as_of,
        order: RotationRunOrder::Volume,
        volume_usd: Decimal::ONE,
        started_at: as_of - TimeDelta::hours(1),
        pool_address: crate::testing::pk(1),
    }
}

async fn page_after(
    repo: &Arc<MockRotationRunRepo>,
    cursor: Option<RotationRunCursor>,
) -> Result<(), RotationRunsError> {
    let scope = RotationRunScope::Protocol(Protocol::MeteoraDammV2);
    service(repo)
        .list_runs(
            Window::Day,
            scope,
            Protocol::MeteoraDammV2,
            RotationRunOrder::Volume,
            cursor,
            50,
        )
        .await
        .map(|_| ())
}

/// Mutation: a later page read through the last tranche written instead of its cursor's end —
/// the window slides between pages, a run started in between listed on a later page.
#[tokio::test]
async fn a_later_page_keeps_the_end_of_its_cursor() {
    let through = minutes(at_october(20, 13), 20);
    let repo = Arc::new(MockRotationRunRepo {
        written_through: Some(through),
        measured_since: Some(at_october(1, 0)),
        ..MockRotationRunRepo::default()
    });

    page_after(&repo, None).await.unwrap();
    page_after(&repo, Some(cursor_ending(at_october(20, 13))))
        .await
        .unwrap();

    let ends: Vec<_> = repo
        .lists
        .lock()
        .unwrap()
        .iter()
        .map(|(_, p)| p.as_of)
        .collect();
    assert_eq!(ends, [through, at_october(20, 13)]);
}

/// Mutation: the end checked against the clock, not the last tranche written, or the margin
/// dropped — a writer two hours behind leaves every next page refused; or a forged end, past
/// the last tranche or long before it, served.
#[tokio::test]
async fn a_cursor_end_is_bound_to_the_last_tranche_written_not_the_clock() {
    // Two hours behind the clock: a writer catching up.
    let through = chrono::Utc::now() - TimeDelta::hours(2);
    let repo = Arc::new(MockRotationRunRepo {
        written_through: Some(through),
        measured_since: Some(at_october(1, 0)),
        ..MockRotationRunRepo::default()
    });

    let read_lately =
        page_after(&repo, Some(cursor_ending(through - TimeDelta::minutes(50)))).await;
    let past_the_writer =
        page_after(&repo, Some(cursor_ending(through + TimeDelta::minutes(5)))).await;
    let too_old = page_after(&repo, Some(cursor_ending(through - TimeDelta::minutes(65)))).await;

    assert!(read_lately.is_ok(), "{read_lately:?}");
    assert!(matches!(
        past_the_writer,
        Err(RotationRunsError::CursorRefused)
    ));
    assert!(matches!(too_old, Err(RotationRunsError::CursorRefused)));
}

/// Mutation: the bars split from the first measured tranche, not the first whole hour a gap
/// after it — an hour half measured, or holding a run cut by the start, split as whole.
#[tokio::test]
async fn the_bars_split_from_the_first_whole_hour_measured() {
    // A gap after 09:50 is 10:20, inside the hour of 10:00: the split starts at 11:00.
    let inside_an_hour = Arc::new(MockRotationRunRepo {
        measured_since: Some(minutes(at_october(19, 9), 50)),
        ..MockRotationRunRepo::default()
    });
    let on_the_hour = Arc::new(MockRotationRunRepo {
        measured_since: Some(minutes(at_october(19, 9), 30)),
        ..MockRotationRunRepo::default()
    });
    let protocol = Protocol::MeteoraDammV2;

    service(&inside_an_hour)
        .list_buckets(Window::Day, BucketPeriod::Hour, protocol)
        .await
        .unwrap();
    service(&on_the_hour)
        .list_buckets(Window::Day, BucketPeriod::Day, protocol)
        .await
        .unwrap();

    assert_eq!(
        *inside_an_hour.splits_since.lock().unwrap(),
        [Some(at_october(19, 11))]
    );
    assert_eq!(
        *on_the_hour.splits_since.lock().unwrap(),
        [Some(at_october(19, 10))]
    );
}
