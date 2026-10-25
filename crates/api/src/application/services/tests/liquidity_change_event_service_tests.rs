//! Unit tests for `LiquidityChangeEventService`: the start of the window a page reads from.

use std::sync::Arc;

use chrono::{Duration, Utc};
use yog_core::domain::{EventCursor, EventScope, Window};

use super::LiquidityChangeEventService;
use crate::testing::{MockLiquidityChangeEventRepo, ts};

/// Mutation: the window's start recomputed on every page — a later page's start slides with
/// the clock and drops the changes between.
#[tokio::test]
async fn a_later_page_keeps_the_start_of_its_cursor() {
    let repo = Arc::new(MockLiquidityChangeEventRepo::default());
    let svc = LiquidityChangeEventService::new(repo.clone());
    let cursor = EventCursor {
        scope: EventScope::All,
        from: ts(1_000),
        at: ts(2_000),
        signature: solana_signature::Signature::from([1; 64]),
        event_index: 0,
    };

    let before = Utc::now();
    svc.list_events(EventScope::All, Window::Month, None, 50)
        .await
        .unwrap();
    svc.list_events(EventScope::All, Window::Month, Some(cursor), 50)
        .await
        .unwrap();

    let starts = repo.windows_from.lock().unwrap().clone();
    assert!(starts[0] >= before - Duration::days(30), "{starts:?}");
    assert!(starts[0] <= Utc::now() - Duration::days(30));
    assert_eq!(starts[1], ts(1_000));
}
