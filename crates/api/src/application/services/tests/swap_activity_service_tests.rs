//! Unit tests for `SwapActivityService`: the period each read covers, and the reads a period not
//! covered never makes.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{Protocol, TokenActivityCursor, Window};

use super::{SwapActivityService, TokenActivityRead};
use crate::application::services::period::{PeriodBounds, ReadPeriod, SeriesInterval};
use crate::testing::{
    MockProtocolActivityRepo, MockSwapActivityRepo, MockTokenActivityRepo, at_october,
    frozen_hours_repo, make_frozen_hours, pk,
};
use yog_core::domain::{FrozenHours, TranchePeriod, WrittenTranches};

fn service(repo: &Arc<MockSwapActivityRepo>) -> SwapActivityService {
    SwapActivityService::new(
        frozen_hours_repo(Some(make_frozen_hours())),
        repo.clone(),
        Arc::new(MockTokenActivityRepo::default()),
    )
}

/// A service whose last ranking of tokens was written at `hour`.
fn ranked(hour: DateTime<Utc>) -> (SwapActivityService, Arc<MockTokenActivityRepo>) {
    let tokens = Arc::new(MockTokenActivityRepo {
        ranked_at: Some(hour),
        ..MockTokenActivityRepo::default()
    });
    let svc = SwapActivityService::new(
        frozen_hours_repo(Some(make_frozen_hours())),
        Arc::new(MockSwapActivityRepo::default()),
        tokens.clone(),
    );
    (svc, tokens)
}

/// Mutation: the `is_covered` guard dropped — a 30-day total over seven days of facts.
#[tokio::test]
async fn a_swap_total_over_hours_the_facts_lack_is_not_read() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let svc = service(&repo);

    let month = svc
        .get_swap_totals(Window::Month, None, None)
        .await
        .unwrap();
    let week = svc
        .get_swap_totals(Window::Week, None, Some(Protocol::MeteoraDammV2))
        .await
        .unwrap();

    assert!(month.measured.items.is_empty());
    let Some(ReadPeriod::Frozen(week)) = week.measured.period else {
        panic!("a week is a frozen period");
    };
    assert_eq!(week.as_of, at_october(20, 13));
    assert_eq!(
        *repo.reads.lock().unwrap(),
        vec![("swap_totals", week, Some(Protocol::MeteoraDammV2))]
    );
}

/// Mutation: the tokens' period built from a separate read of the last frozen hour — a freeze
/// in between, and the page names an hour it was not ranked at.
#[tokio::test]
async fn the_tokens_span_ends_at_the_hour_of_their_ranking() {
    let (svc, tokens) = ranked(at_october(20, 11));

    let TokenActivityRead::Page(page) = svc
        .list_token_activity(Window::Week, None, 10)
        .await
        .unwrap()
    else {
        panic!("no cursor, no expiry");
    };

    assert_eq!(page.period.unwrap().as_of(), at_october(20, 12));
    assert_eq!(*tokens.reads.lock().unwrap(), [Window::Week]);
}

/// Mutation: the cursor's hour unchecked — page 2 of one ranking read in the next.
#[tokio::test]
async fn a_cursor_of_a_ranking_rewritten_since_is_expired() {
    let (svc, _) = ranked(at_october(20, 12));
    let cursor = |hour| TokenActivityCursor {
        last_hour: hour,
        volume_usd: Decimal::from(5),
        mint: pk(1),
    };

    let stale = svc
        .list_token_activity(Window::Day, Some(cursor(at_october(20, 11))), 10)
        .await
        .unwrap();
    let fresh = svc
        .list_token_activity(Window::Day, Some(cursor(at_october(20, 12))), 10)
        .await
        .unwrap();

    assert!(matches!(stale, TokenActivityRead::CursorExpired));
    assert!(matches!(fresh, TokenActivityRead::Page(_)));
}

/// Tranches written from the 13th of October, the last ending at 20 October 12:40.
fn tranches() -> WrittenTranches {
    WrittenTranches {
        first: at_october(13, 13),
        through: at_october(20, 12) + chrono::TimeDelta::minutes(40),
    }
}

fn in_hour(
    repo: &Arc<MockSwapActivityRepo>,
    tokens: &Arc<MockTokenActivityRepo>,
) -> SwapActivityService {
    SwapActivityService::new(
        Arc::new(MockProtocolActivityRepo::with_tranches(tranches())),
        repo.clone(),
        tokens.clone(),
    )
}

/// Mutation: 1h read as a frozen period, or its previous hour not moved back — the last hour is
/// the one through the last tranche written, and `offset=1h` the hour before it.
#[tokio::test]
async fn the_last_hour_is_read_through_the_last_tranche() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let svc = in_hour(&repo, &Arc::new(MockTokenActivityRepo::default()));

    svc.get_swap_totals(Window::Hour, None, None).await.unwrap();
    svc.get_swap_totals(Window::Hour, Some(Window::Hour), None)
        .await
        .unwrap();
    svc.list_swap_series(Window::Hour, SeriesInterval::FiveMinutes, None)
        .await
        .unwrap();

    let hour = TranchePeriod::hour_through(tranches(), None);
    let before = TranchePeriod::hour_through(tranches(), Some(Window::Hour));
    assert_eq!(
        *repo.tranche_reads.lock().unwrap(),
        vec![
            ("swap_totals", hour, None),
            ("swap_totals", before, None),
            ("swap_totals", before, None),
            ("swap_tranches", hour, None),
        ]
    );
    assert!(repo.reads.lock().unwrap().is_empty());
}

/// Mutation: an `interval` read at another step — `1h` read as the days, `1d` as the hours.
#[tokio::test]
async fn a_series_reads_the_frozen_buckets_of_its_interval() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let svc = service(&repo);

    svc.list_swap_series(Window::Week, SeriesInterval::Hour, None)
        .await
        .unwrap();
    svc.list_swap_series(Window::Week, SeriesInterval::Day, None)
        .await
        .unwrap();

    let reads: Vec<_> = repo.reads.lock().unwrap().iter().map(|r| r.0).collect();
    assert_eq!(reads, vec!["swap_hourly", "swap_buckets"]);
}

/// Mutation: the hour's ranking read without its cursor checked — page 2 of a ranking read once a
/// tranche has moved it.
#[tokio::test]
async fn a_cursor_of_an_hour_moved_since_is_expired() {
    let tokens = Arc::new(MockTokenActivityRepo::default());
    let svc = in_hour(&Arc::new(MockSwapActivityRepo::default()), &tokens);
    let cursor = |end| TokenActivityCursor {
        last_hour: end,
        volume_usd: Decimal::from(5),
        mint: pk(1),
    };
    let hour = TranchePeriod::hour_through(tranches(), None);

    let stale = svc
        .list_token_activity(
            Window::Hour,
            Some(cursor(hour.as_of - chrono::TimeDelta::minutes(5))),
            10,
        )
        .await
        .unwrap();
    let fresh = svc
        .list_token_activity(Window::Hour, Some(cursor(hour.as_of)), 10)
        .await
        .unwrap();

    assert!(matches!(stale, TokenActivityRead::CursorExpired));
    assert!(matches!(fresh, TokenActivityRead::Page(_)));
    assert_eq!(*tokens.tranche_reads.lock().unwrap(), vec![hour]);
}

/// Mutation: a slice of 6 hours read at another width — bars of a day under `interval=6h`.
#[tokio::test]
async fn a_week_is_read_in_slices_of_six_hours() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let svc = service(&repo);

    svc.list_swap_series(Window::Week, SeriesInterval::SixHours, None)
        .await
        .unwrap();
    svc.list_pool_swap_series(Window::Week, SeriesInterval::SixHours, pk(1))
        .await
        .unwrap();

    assert_eq!(
        *repo.widths.lock().unwrap(),
        [chrono::TimeDelta::hours(6); 2]
    );
}

/// Mutation: the previous window read on its own end — two reads a freeze can split.
#[tokio::test]
async fn the_previous_window_ends_where_the_window_starts() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let hours = FrozenHours {
        first: at_october(1, 0),
        last_frozen: at_october(20, 12),
    };
    let svc = SwapActivityService::new(
        frozen_hours_repo(Some(hours)),
        repo.clone(),
        Arc::new(MockTokenActivityRepo::default()),
    );

    let totals = svc.get_swap_totals(Window::Week, None, None).await.unwrap();

    let previous = totals.previous.unwrap();
    assert!(previous.covered);
    let reads = repo.reads.lock().unwrap();
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[1].1.as_of, reads[0].1.from);
    assert_eq!(reads[1].1.from, reads[0].1.from - Window::Week.duration());
}

/// Mutation: an uncovered previous window summed anyway — a change over part of a window.
#[tokio::test]
async fn a_previous_window_the_facts_do_not_cover_is_said_and_not_read() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let svc = service(&repo);

    let totals = svc.get_swap_totals(Window::Week, None, None).await.unwrap();

    let previous = totals.previous.unwrap();
    assert!(!previous.covered);
    assert_eq!(previous.volume_usd, None);
    assert_eq!(repo.reads.lock().unwrap().len(), 1);
}

/// Mutation: a read moved back given a previous window of its own — a third read nobody asked for.
#[tokio::test]
async fn a_read_moved_back_has_no_previous_window() {
    let repo = Arc::new(MockSwapActivityRepo::default());
    let svc = service(&repo);

    let totals = svc
        .get_swap_totals(Window::Day, Some(Window::Day), None)
        .await
        .unwrap();

    assert!(totals.previous.is_none());
    assert_eq!(repo.reads.lock().unwrap().len(), 1);
}
