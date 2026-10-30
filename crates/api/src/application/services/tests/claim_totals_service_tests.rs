//! Unit tests for `ClaimTotalsService`: the claims' own coverage, from the first whole hour
//! valued, and the claims each total reads over a covered window.

use std::sync::Arc;

use chrono::TimeDelta;
use yog_core::domain::{ClaimKind, Protocol, Window, WrittenTranches};

use super::ClaimTotalsService;
use crate::application::services::period::PeriodBounds;
use crate::application::services::period::SeriesInterval;
use crate::testing::{
    MockClaimTotalsRepo, MockProtocolActivityRepo, at_october, frozen_hours_repo, make_frozen_hours,
};

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
        .get_claim_totals(ClaimKind::PositionFee, Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();
    let not_yet = service(&unmeasured)
        .get_claim_totals(ClaimKind::PositionFee, Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(day.period.unwrap().covered_since(), at_october(20, 1));
    assert!(
        day.items.is_empty() && reads(&measured).is_empty(),
        "{:?}",
        reads(&measured)
    );
    assert_eq!(
        not_yet.period.unwrap().covered_since(),
        not_yet.period.unwrap().as_of()
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
        .get_claim_totals(ClaimKind::PositionFee, Window::Day, Protocol::MeteoraDammV2)
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
        .get_claim_totals(ClaimKind::Reward, Window::Day, Protocol::MeteoraDammV2)
        .await
        .unwrap();

    assert_eq!(totals.items.len(), 1);
    assert_eq!(reads(&repo), ["reward_claim_totals"]);
}

/// Mutation: a series of one kind read from the other's claims, or at another step.
#[tokio::test]
async fn a_series_reads_its_claims_at_its_interval() {
    let repo = covering();
    let hours = MockProtocolActivityRepo::with_hours_and_tranches(
        make_frozen_hours(),
        WrittenTranches {
            first: at_october(13, 13),
            through: at_october(20, 12) + chrono::TimeDelta::minutes(40),
        },
    );
    let svc = ClaimTotalsService::new(Arc::new(hours), repo.clone());

    for (kind, window, interval) in [
        (ClaimKind::PositionFee, Window::Day, SeriesInterval::Hour),
        (ClaimKind::Reward, Window::Week, SeriesInterval::Day),
        (ClaimKind::Reward, Window::Hour, SeriesInterval::FiveMinutes),
    ] {
        svc.list_claim_series(kind, window, interval, Protocol::MeteoraDammV2)
            .await
            .unwrap();
    }

    assert_eq!(
        reads(&repo),
        ["position_fee_claim_hourly", "reward_claim_buckets"]
    );
    let tranches = repo.tranche_reads.lock().unwrap();
    assert_eq!(tranches.len(), 1);
    assert_eq!(
        (tranches[0].0, tranches[0].1),
        ("claim_tranches", ClaimKind::Reward)
    );
}

/// Mutation: a slice of 6 hours read at another width — bars of a day under `interval=6h`.
#[tokio::test]
async fn a_week_is_read_in_slices_of_six_hours() {
    let repo = covering();

    service(&repo)
        .list_claim_series(
            ClaimKind::Reward,
            Window::Week,
            SeriesInterval::SixHours,
            Protocol::MeteoraDammV2,
        )
        .await
        .unwrap();

    assert_eq!(*repo.widths.lock().unwrap(), [TimeDelta::hours(6)]);
}
