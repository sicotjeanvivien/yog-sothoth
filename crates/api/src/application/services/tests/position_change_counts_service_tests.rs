//! Unit tests for `PositionChangeCountsService`: the reads a window not covered never makes.

use std::sync::Arc;

use yog_core::domain::{Protocol, Window, WrittenTranches};

use super::PositionChangeCountsService;
use crate::application::services::period::SeriesInterval;
use crate::testing::{
    MockPositionChangeCountsRepo, MockProtocolActivityRepo, at_october, frozen_hours_repo,
    make_frozen_hours,
};

/// Mutation: the `is_covered` guard dropped — a 30-day count over seven days of facts.
#[tokio::test]
async fn the_counts_over_hours_the_facts_lack_are_not_read() {
    let repo = Arc::new(MockPositionChangeCountsRepo::default());
    let svc = PositionChangeCountsService::new(
        frozen_hours_repo(Some(make_frozen_hours())),
        repo.clone(),
    );

    let month = svc
        .get_position_change_totals(Window::Month, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let week = svc
        .get_position_change_totals(Window::Week, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert!(month.items.is_empty());
    assert_eq!(week.items.len(), 1);
    assert_eq!(repo.reads.lock().unwrap().len(), 1);
}

/// Mutation: an `interval` read at another step — `1h` as the days, `1d` as the hours, `5m` as
/// the hours.
#[tokio::test]
async fn a_series_reads_the_buckets_of_its_interval() {
    let repo = Arc::new(MockPositionChangeCountsRepo::default());
    let hours = MockProtocolActivityRepo::with_hours_and_tranches(
        make_frozen_hours(),
        WrittenTranches {
            first: at_october(13, 13),
            through: at_october(20, 12) + chrono::TimeDelta::minutes(40),
        },
    );
    let svc = PositionChangeCountsService::new(Arc::new(hours), repo.clone());

    for (window, interval) in [
        (Window::Day, SeriesInterval::Hour),
        (Window::Week, SeriesInterval::Day),
        (Window::Hour, SeriesInterval::FiveMinutes),
    ] {
        svc.list_position_change_series(window, interval, Protocol::MeteoraDammV2)
            .await
            .unwrap();
    }

    assert_eq!(
        *repo.series_reads.lock().unwrap(),
        ["position_hourly", "position_daily", "position_tranches"]
    );
}
