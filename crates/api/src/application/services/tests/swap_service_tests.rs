//! Unit tests for `SwapService`: the period each read covers, and the reads a period not
//! covered never makes.

use std::sync::Arc;

use yog_core::domain::{Protocol, Window};

use super::SwapService;
use crate::testing::{MockSwapRepo, at_october, frozen_hours_repo, make_frozen_hours};

/// Mutation: the `is_covered` guard dropped — a 30-day total over seven days of facts.
#[tokio::test]
async fn a_swap_total_over_hours_the_facts_lack_is_not_read() {
    let repo = Arc::new(MockSwapRepo::default());
    let svc = SwapService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone());

    let month = svc
        .get_swap_totals(Window::Month, None, None)
        .await
        .unwrap();
    let week = svc
        .get_swap_totals(Window::Week, None, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert!(month.items.is_empty());
    assert_eq!(week.period.unwrap().as_of, at_october(20, 13));
    assert_eq!(
        *repo.reads.lock().unwrap(),
        vec![(
            "swap_totals",
            week.period.unwrap(),
            Some(Protocol::MeteoraDammV2)
        )]
    );
}
