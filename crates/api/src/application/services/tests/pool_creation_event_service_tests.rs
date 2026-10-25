//! Unit tests for `PoolCreationEventService`: the start of the window a page reads from.

use std::sync::Arc;

use chrono::{Duration, Utc};
use yog_core::domain::{EventCursor, EventScope, Window};

use super::PoolCreationEventService;
use crate::testing::{MockPoolCreationEventRepo, ts};

/// Mutation: the window's start recomputed on every page — a later page's start slides with
/// the clock and drops the creations between.
#[tokio::test]
async fn a_later_page_keeps_the_start_of_its_cursor() {
    let repo = Arc::new(MockPoolCreationEventRepo::default());
    let svc = PoolCreationEventService::new(repo.clone());
    let cursor = EventCursor {
        scope: EventScope::All,
        from: ts(1_000),
        at: ts(2_000),
        signature: solana_signature::Signature::from([1; 64]),
        event_index: 0,
    };

    let before = Utc::now();
    svc.list_events(None, Window::Month, None, 50)
        .await
        .unwrap();
    svc.list_events(None, Window::Month, Some(cursor), 50)
        .await
        .unwrap();

    let starts = repo.windows_from.lock().unwrap().clone();
    assert!(starts[0] >= before - Duration::days(30), "{starts:?}");
    assert!(starts[0] <= Utc::now() - Duration::days(30));
    assert_eq!(starts[1], ts(1_000));
}
