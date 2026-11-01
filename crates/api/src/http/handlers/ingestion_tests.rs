use std::sync::Arc;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use serde_json::json;

use crate::application::IngestionService;
use crate::testing::{MockEventFreshnessRepo, app_state, call_on};

async fn read(repo: MockEventFreshnessRepo) -> serde_json::Value {
    let mut state = app_state();
    state.ingestion_service = Arc::new(IngestionService::new(Arc::new(repo)));

    let (status, _, body) = call_on(state, Method::GET, "/api/ingestion").await;

    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str(&body).unwrap()
}

/// Mutation: the date dropped from the response, or the verdict not the service's — the dashboard's
/// dot saying another state than the last event gives.
#[tokio::test]
async fn a_recent_event_is_served_live_with_its_date() {
    let at = Utc::now();

    assert_eq!(
        read(MockEventFreshnessRepo::at(at)).await,
        json!({ "freshness": "live", "lastEventAt": at })
    );
}

/// Mutation: no event answered as an error, or as live — a new database said to be up to date.
#[tokio::test]
async fn no_event_is_served_stale_without_a_date() {
    assert_eq!(
        read(MockEventFreshnessRepo::never()).await,
        json!({ "freshness": "stale", "lastEventAt": null })
    );
}
