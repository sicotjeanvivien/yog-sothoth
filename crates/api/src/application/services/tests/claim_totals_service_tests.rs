//! Unit tests for `ClaimTotalsService`: the claims' own coverage, from the first whole hour
//! valued, and the claims each total reads over a covered window.

use std::sync::Arc;

use chrono::TimeDelta;
use yog_core::domain::{Protocol, Window};

use super::ClaimTotalsService;
use crate::testing::{MockClaimTotalsRepo, at_october, frozen_hours_repo, make_frozen_hours};

fn service(repo: &Arc<MockClaimTotalsRepo>) -> ClaimTotalsService {
    ClaimTotalsService::new(frozen_hours_repo(Some(make_frozen_hours())), repo.clone())
}

fn reads(repo: &MockClaimTotalsRepo) -> Vec<&'static str> {
    repo.reads.lock().unwrap().iter().map(|r| r.0).collect()
}

/// Mutation: the coverage taken from the tranche itself — an hour the claims start inside summed
/// as whole; or unmeasured claims covered from the facts' first hour.
#[tokio::test]
async fn the_claims_are_covered_from_the_first_whole_hour_valued() {
    let measured = Arc::new(MockClaimTotalsRepo {
        since: Some(at_october(20, 0) + TimeDelta::minutes(5)),
        ..MockClaimTotalsRepo::default()
    });
    let unmeasured = Arc::new(MockClaimTotalsRepo::default());

    let day = service(&measured)
        .get_position_fee_claim_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let not_yet = service(&unmeasured)
        .get_position_fee_claim_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(day.period.unwrap().covered_since, at_october(20, 1));
    assert!(
        day.items.is_empty() && reads(&measured).is_empty(),
        "{:?}",
        reads(&measured)
    );
    assert_eq!(
        not_yet.period.unwrap().covered_since,
        not_yet.period.unwrap().as_of
    );
    assert!(reads(&unmeasured).is_empty());
}

fn covering() -> Arc<MockClaimTotalsRepo> {
    Arc::new(MockClaimTotalsRepo {
        since: Some(at_october(13, 13)),
        ..MockClaimTotalsRepo::default()
    })
}

/// Mutation: the fees' totals read from the rewards.
#[tokio::test]
async fn the_fees_totals_read_the_fee_claims() {
    let repo = covering();

    let totals = service(&repo)
        .get_position_fee_claim_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(totals.items.len(), 1);
    assert_eq!(reads(&repo), ["position_fee_claim_totals"]);
}

/// Mutation: the rewards' totals read from the fees.
#[tokio::test]
async fn the_rewards_totals_read_the_reward_claims() {
    let repo = covering();

    let totals = service(&repo)
        .get_reward_claim_totals(Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(totals.items.len(), 1);
    assert_eq!(reads(&repo), ["reward_claim_totals"]);
}
