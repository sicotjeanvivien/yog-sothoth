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
