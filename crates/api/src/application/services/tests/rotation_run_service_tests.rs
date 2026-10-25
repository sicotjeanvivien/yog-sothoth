//! Unit tests for `RotationRunService`: the coverage of the lists, the totals and the bars, and
//! the end a later page keeps.

use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{Protocol, RotationRunCursor, RotationRunOrder, RotationRunScope, Window};

use super::RotationRunService;
use crate::application::BucketPeriod;
use crate::testing::{MockRotationRunRepo, at_october, frozen_hours_repo, make_frozen_hours};

fn service(repo: &Arc<MockRotationRunRepo>) -> RotationRunService {
    RotationRunService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone())
}

fn minutes(at: DateTime<Utc>, minutes: i64) -> DateTime<Utc> {
    at + TimeDelta::minutes(minutes)
}

/// Mutation: the coverage dropped, or taken from the facts — a window the runs start inside
/// summed as whole, or an unmeasured protocol covered.
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
        minutes(at_october(19, 10), 5)
    );
    assert!(not_yet.items.is_empty());
}

/// Mutation: a later page read through the last tranche written instead of its cursor's end —
/// the window slides between pages, and a run is skipped or repeated.
#[tokio::test]
async fn a_later_page_keeps_the_end_of_its_cursor() {
    let repo = Arc::new(MockRotationRunRepo {
        written_through: Some(minutes(at_october(20, 13), 20)),
        measured_since: Some(at_october(1, 0)),
        ..MockRotationRunRepo::default()
    });
    let scope = RotationRunScope::Protocol(Protocol::MeteoraDammV2);
    let cursor = RotationRunCursor {
        scope,
        window: Window::Day,
        as_of: at_october(20, 12),
        order: RotationRunOrder::Volume,
        volume_usd: Decimal::ONE,
        started_at: at_october(20, 1),
        pool_address: crate::testing::pk(1),
    };
    let svc = service(&repo);

    svc.list_runs(
        Window::Day,
        scope,
        Protocol::MeteoraDammV2,
        RotationRunOrder::Volume,
        None,
        50,
    )
    .await
    .unwrap();
    svc.list_runs(
        Window::Day,
        scope,
        Protocol::MeteoraDammV2,
        RotationRunOrder::Volume,
        Some(cursor),
        50,
    )
    .await
    .unwrap();

    let ends: Vec<_> = repo
        .lists
        .lock()
        .unwrap()
        .iter()
        .map(|(_, p)| p.as_of)
        .collect();
    assert_eq!(ends, [minutes(at_october(20, 13), 20), at_october(20, 12)]);
}

/// Mutation: the bars covered from the first measured tranche, not its first whole hour — an
/// hour half measured drawn as whole.
#[tokio::test]
async fn the_bars_start_at_the_first_whole_hour_measured() {
    let inside_an_hour = Arc::new(MockRotationRunRepo {
        measured_since: Some(minutes(at_october(19, 10), 5)),
        ..MockRotationRunRepo::default()
    });
    let on_the_hour = Arc::new(MockRotationRunRepo {
        measured_since: Some(at_october(19, 10)),
        ..MockRotationRunRepo::default()
    });
    let protocol = Protocol::MeteoraDammV2;

    let inside = service(&inside_an_hour)
        .list_buckets(Window::Day, BucketPeriod::Hour, protocol)
        .await
        .unwrap();
    let on = service(&on_the_hour)
        .list_buckets(Window::Day, BucketPeriod::Hour, protocol)
        .await
        .unwrap();

    assert_eq!(inside.period.unwrap().covered_since, at_october(19, 11));
    assert_eq!(on.period.unwrap().covered_since, at_october(19, 10));
}
