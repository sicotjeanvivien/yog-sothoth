//! Unit tests for `PoolCreationsService`: the reads a period not covered never makes, and what
//! its series reads.

use std::sync::Arc;

use yog_core::domain::Window;

use super::PoolCreationsService;
use crate::application::services::period::SeriesInterval;
use crate::testing::{
    MockPoolCreationsRepo, MockProtocolActivityRepo, frozen_hours_repo, make_frozen_hours,
    tranches_begun,
};

/// Mutation: a slice of 6 hours read at another width — bars of a day under `interval=6h`.
#[tokio::test]
async fn a_week_is_read_in_slices_of_six_hours() {
    let repo = Arc::new(MockPoolCreationsRepo::default());
    let svc = PoolCreationsService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    svc.list_pool_creation_series(Window::Week, SeriesInterval::SixHours, None)
        .await
        .unwrap();

    assert_eq!(*repo.widths.lock().unwrap(), [chrono::TimeDelta::hours(6)]);
}

/// Mutation: a frozen guard forced either way — thirty days summed over seven, or a week never.
#[tokio::test]
async fn the_totals_are_read_only_over_a_covered_window() {
    let repo = Arc::new(MockPoolCreationsRepo::default());
    let svc = PoolCreationsService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    svc.get_pool_creation_totals(Window::Month, None)
        .await
        .unwrap();
    assert!(repo.reads.lock().unwrap().is_empty());

    svc.get_pool_creation_totals(Window::Week, None)
        .await
        .unwrap();
    assert_eq!(*repo.reads.lock().unwrap(), ["totals"]);
}

/// Mutation: the tranche guard forced either way — half an hour summed as an hour, or a covered
/// hour never read.
#[tokio::test]
async fn the_totals_of_the_hour_are_read_only_when_its_tranches_cover_it() {
    let read = async |tranches| {
        let repo = Arc::new(MockPoolCreationsRepo::default());
        let hours = Arc::new(MockProtocolActivityRepo::with_tranches(tranches));
        PoolCreationsService::new(hours, repo.clone())
            .get_pool_creation_totals(Window::Hour, None)
            .await
            .unwrap();
        repo.reads.lock().unwrap().clone()
    };

    assert!(read(tranches_begun(30)).await.is_empty());
    assert_eq!(read(tranches_begun(120)).await, ["totals_in_tranches"]);
}

/// Mutation: an `interval` read at another step — `5m` or `1h` read as other buckets.
#[tokio::test]
async fn a_series_reads_the_buckets_of_its_interval() {
    let repo = Arc::new(MockPoolCreationsRepo::default());
    let hours = Arc::new(MockProtocolActivityRepo::with_hours_and_tranches(
        make_frozen_hours(),
        tranches_begun(120),
    ));
    let svc = PoolCreationsService::new(hours, repo.clone());

    for (window, interval) in [
        (Window::Hour, SeriesInterval::FiveMinutes),
        (Window::Week, SeriesInterval::Hour),
        (Window::Week, SeriesInterval::Day),
    ] {
        svc.list_pool_creation_series(window, interval, None)
            .await
            .unwrap();
    }

    assert_eq!(
        *repo.reads.lock().unwrap(),
        ["tranches", "hourly", "buckets"]
    );
}
