//! Unit tests for `LiquidityTurnoverService`: what each of its reads covers.

use std::sync::Arc;

use yog_core::domain::{Protocol, Window};

use super::LiquidityTurnoverService;
use crate::testing::{
    MockProtocolTvlRepo, MockSwapActivityRepo, frozen_hours_repo, make_frozen_hours,
};

/// Mutation: a TVL read, or a volume, for a window the facts do not cover or that has no row.
#[tokio::test]
async fn a_window_without_a_row_reads_no_tvl() {
    let swaps = Arc::new(MockSwapActivityRepo::default());
    let tvl = Arc::new(MockProtocolTvlRepo::default());
    let svc = LiquidityTurnoverService::new(
        frozen_hours_repo(Some(make_frozen_hours())),
        swaps.clone(),
        tvl.clone(),
    );

    let month = svc.get_totals(Window::Month, None).await.unwrap();
    assert!(month.measured.items.is_empty());
    assert_eq!(month.total, None);
    assert!(swaps.reads.lock().unwrap().is_empty());

    let week = svc
        .get_totals(Window::Week, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();
    assert_eq!(swaps.reads.lock().unwrap().len(), 1);
    assert!(week.measured.items.is_empty());
    assert!(tvl.instants.lock().unwrap().is_empty());
}
