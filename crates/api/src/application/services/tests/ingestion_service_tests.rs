//! Unit tests for `IngestionService`.

use std::sync::Arc;

use chrono::Utc;
use yog_core::{RepositoryError, domain::FreshnessStatus};

use super::super::IngestionService;
use crate::testing::{MockEventFreshnessRepo, ts};

#[tokio::test]
async fn freshness_is_stale_when_no_event_indexed() {
    let svc = IngestionService::new(Arc::new(MockEventFreshnessRepo::never()));

    let agg = svc.freshness().await.unwrap();

    assert!(agg.last_event_at.is_none());
    assert!(matches!(agg.freshness, FreshnessStatus::Stale));
}

#[tokio::test]
async fn a_recent_event_is_live_and_propagated() {
    let event_ts = Utc::now();
    let svc = IngestionService::new(Arc::new(MockEventFreshnessRepo::at(event_ts)));

    let agg = svc.freshness().await.unwrap();

    assert_eq!(agg.last_event_at, Some(event_ts));
    assert!(matches!(agg.freshness, FreshnessStatus::Live));
}

#[tokio::test]
async fn an_old_event_is_stale() {
    let svc = IngestionService::new(Arc::new(MockEventFreshnessRepo::at(ts(1_999))));

    let agg = svc.freshness().await.unwrap();

    assert!(matches!(agg.freshness, FreshnessStatus::Stale));
}

#[tokio::test]
async fn repo_error_propagates() {
    let svc = IngestionService::new(Arc::new(MockEventFreshnessRepo::failing()));

    assert!(matches!(
        svc.freshness().await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}
