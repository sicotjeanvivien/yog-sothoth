//! Unit tests for `SwapActivityService`: the period each read covers, and the reads a period not
//! covered never makes.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{Protocol, TokenActivityCursor, Window};

use super::{SwapActivityService, TokenActivityRead};
use crate::testing::{
    MockSwapActivityRepo, MockTokenActivityRepo, at_october, frozen_hours_repo, make_frozen_hours,
    pk,
};

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

    assert_eq!(page.period.unwrap().as_of, at_october(20, 12));
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
