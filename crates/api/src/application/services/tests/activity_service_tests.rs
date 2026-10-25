//! Unit tests for `ActivityService`: the period each read covers, and the reads
//! a period the facts do not cover never makes.

use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use rust_decimal::Decimal;
use yog_core::domain::{FrozenHours, TokenActivityCursor, Window};

use super::super::{ActivityService, TokenActivityRead};
use crate::testing::{MockProtocolActivityRepo, MockTokenActivityRepo, pk};

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).unwrap()
}

/// Facts from the 13th at 13:00 through the 20th at 12:00: 24h and 7d are covered, 30d is not.
fn hours() -> FrozenHours {
    FrozenHours {
        first: at(13, 13),
        last_frozen: at(20, 12),
    }
}

/// The last ranking of tokens, written at `hour`.
fn ranked(hour: DateTime<Utc>) -> MockTokenActivityRepo {
    MockTokenActivityRepo {
        ranked_at: Some(hour),
        ..MockTokenActivityRepo::default()
    }
}

fn service(
    protocols: &Arc<MockProtocolActivityRepo>,
    tokens: &Arc<MockTokenActivityRepo>,
) -> ActivityService {
    ActivityService::new(protocols.clone(), tokens.clone())
}

/// Mutation: the `is_covered` guard dropped — a 30-day total over seven days of facts.
#[tokio::test]
async fn a_total_over_hours_the_facts_lack_is_not_read() {
    let protocols = Arc::new(MockProtocolActivityRepo::new(Some(hours())));
    let tokens = Arc::new(ranked(at(20, 12)));
    let svc = service(&protocols, &tokens);

    let month = svc.protocol_activity(Window::Month, None).await.unwrap();
    let TokenActivityRead::Page(tokens_month) =
        svc.token_activity(Window::Month, None, 10).await.unwrap()
    else {
        panic!("no cursor, no expiry");
    };

    assert!(protocols.periods.lock().unwrap().is_empty());
    assert_eq!(month.period.unwrap().as_of, at(20, 13));
    assert!(!tokens_month.period.unwrap().is_covered());
}

/// Mutation: the offset ignored, or the last frozen hour taken as the period's end.
#[tokio::test]
async fn a_total_ends_at_the_close_of_the_last_frozen_hour_moved_by_its_offset() {
    let protocols = Arc::new(MockProtocolActivityRepo::new(Some(hours())));
    let tokens = Arc::new(MockTokenActivityRepo::default());
    let svc = service(&protocols, &tokens);

    svc.protocol_activity(Window::Day, None).await.unwrap();
    svc.protocol_activity(Window::Day, Some(Window::Day))
        .await
        .unwrap();

    let periods = protocols.periods.lock().unwrap();
    assert_eq!(
        (periods[0].from, periods[0].as_of),
        (at(19, 13), at(20, 13))
    );
    assert_eq!(
        (periods[1].from, periods[1].as_of),
        (at(18, 13), at(19, 13))
    );
}

/// Mutation: the tokens' period built from a separate read of the last frozen
/// hour — a freeze in between, and the page names an hour it was not ranked at.
#[tokio::test]
async fn the_tokens_span_ends_at_the_hour_of_their_ranking() {
    let protocols = Arc::new(MockProtocolActivityRepo::new(Some(hours())));
    let tokens = Arc::new(ranked(at(20, 11)));
    let svc = service(&protocols, &tokens);

    let TokenActivityRead::Page(page) = svc.token_activity(Window::Week, None, 10).await.unwrap()
    else {
        panic!("no cursor, no expiry");
    };

    assert_eq!(page.period.unwrap().as_of, at(20, 12));
    assert_eq!(*tokens.reads.lock().unwrap(), [Window::Week]);
}

/// Mutation: the cursor's hour unchecked — page 2 of one ranking read in the next.
#[tokio::test]
async fn a_cursor_of_a_ranking_rewritten_since_is_expired() {
    let protocols = Arc::new(MockProtocolActivityRepo::new(Some(hours())));
    let tokens = Arc::new(ranked(at(20, 12)));
    let svc = service(&protocols, &tokens);
    let cursor = |hour| TokenActivityCursor {
        last_hour: hour,
        volume_usd: Decimal::from(5),
        mint: pk(1),
    };

    let stale = svc
        .token_activity(Window::Day, Some(cursor(at(20, 11))), 10)
        .await
        .unwrap();
    let fresh = svc
        .token_activity(Window::Day, Some(cursor(at(20, 12))), 10)
        .await
        .unwrap();

    assert!(matches!(stale, TokenActivityRead::CursorExpired));
    assert!(matches!(fresh, TokenActivityRead::Page(_)));
}

/// Mutation: the series starting at the window's first hour, which loses the
/// measure the variation starts from.
#[tokio::test]
async fn the_tvl_series_starts_with_the_hour_that_closes_at_the_window_start() {
    let protocols = Arc::new(MockProtocolActivityRepo::new(Some(hours())));
    let tokens = Arc::new(MockTokenActivityRepo::default());
    let svc = service(&protocols, &tokens);

    svc.protocol_tvl_hourly(Window::Day).await.unwrap();
    svc.protocol_tvl().await.unwrap();

    let ranges = protocols.tvl_ranges.lock().unwrap();
    assert_eq!(ranges[0], (at(19, 12), at(20, 12)));
    assert_eq!(
        ranges[1],
        (at(20, 12), at(20, 12)),
        "the latest by the last frozen hour"
    );
}

/// Mutation: a read run with an invented period before any hour is frozen.
#[tokio::test]
async fn before_the_first_freeze_nothing_is_read() {
    let protocols = Arc::new(MockProtocolActivityRepo::new(None));
    let tokens = Arc::new(MockTokenActivityRepo::default());
    let svc = service(&protocols, &tokens);

    let activity = svc.protocol_activity(Window::Day, None).await.unwrap();
    let hourly = svc.protocol_activity_hourly(Window::Day).await.unwrap();
    let tvl = svc.protocol_tvl().await.unwrap();

    assert!(activity.period.is_none() && hourly.period.is_none() && tvl.period.is_none());
    assert!(protocols.periods.lock().unwrap().is_empty());
    assert!(protocols.tvl_ranges.lock().unwrap().is_empty());
}
