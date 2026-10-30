//! Unit tests for `PoolCreationsService`: the width its series reads.

use std::sync::Arc;

use yog_core::domain::Window;

use super::PoolCreationsService;
use crate::application::services::period::SeriesInterval;
use crate::testing::{MockPoolCreationsRepo, frozen_hours_repo, make_frozen_hours};

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
