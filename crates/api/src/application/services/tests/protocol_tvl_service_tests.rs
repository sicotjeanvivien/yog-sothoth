//! Unit tests for `ProtocolTvlService`: the hours its readings are read at.

use std::sync::Arc;

use chrono::Duration;
use yog_core::domain::Window;

use super::ProtocolTvlService;
use crate::testing::{MockProtocolTvlRepo, at_october, frozen_hours_repo, make_frozen_hours};

/// Mutation: the TVL read at the open hour, or its series from the window's first hour — the
/// window's start reading lost.
#[tokio::test]
async fn the_tvl_reads_the_last_frozen_hour_and_the_series_from_the_windows_start() {
    let repo = Arc::new(MockProtocolTvlRepo::default());
    let svc = ProtocolTvlService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    svc.get_tvl(None).await.unwrap();
    let day = svc.list_tvl_hourly(Window::Day, None).await.unwrap();

    let instants = repo.instants.lock().unwrap().clone();
    assert_eq!(instants[0], ("tvl", at_october(20, 12), None));
    assert_eq!(
        instants[1],
        (
            "tvl_hourly",
            day.period.unwrap().from - Duration::hours(1),
            None
        )
    );
}

/// Mutation: the change read at the open hour, or over another window than the one asked — a
/// 7-day request served the 24-hour change.
#[tokio::test]
async fn the_change_reads_the_last_frozen_hour_over_its_window() {
    let repo = Arc::new(MockProtocolTvlRepo::default());
    let svc = ProtocolTvlService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    svc.get_tvl_change(Window::Week, None).await.unwrap();

    let instants = repo.instants.lock().unwrap().clone();
    assert_eq!(instants, vec![("tvl_changes", at_october(20, 12), None)]);
    assert_eq!(*repo.windows.lock().unwrap(), vec![Window::Week]);
}

/// Mutation: the last hour read as a change — a window of an hour looked up among the readings
/// and the discovered parts, which have none.
#[tokio::test]
async fn the_change_over_the_last_hour_is_the_last_reading_alone() {
    let repo = Arc::new(MockProtocolTvlRepo::default());
    let svc = ProtocolTvlService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    svc.get_tvl_change(Window::Hour, None).await.unwrap();

    assert!(repo.windows.lock().unwrap().is_empty());
    assert_eq!(
        repo.instants.lock().unwrap().clone(),
        [("tvl", at_october(20, 12), None)]
    );
}
