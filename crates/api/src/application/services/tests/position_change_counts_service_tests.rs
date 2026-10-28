//! Unit tests for `PositionChangeCountsService`: the reads a window not covered never makes.

use std::sync::Arc;

use yog_core::domain::{Protocol, Window};

use super::PositionChangeCountsService;
use crate::testing::{MockPositionChangeCountsRepo, frozen_hours_repo, make_frozen_hours};

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
