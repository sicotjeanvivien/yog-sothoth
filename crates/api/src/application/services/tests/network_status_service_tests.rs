//! Unit tests for `NetworkStatusService`.

use std::sync::Arc;

use yog_core::{RepositoryError, domain::FreshnessStatus};

use super::super::NetworkStatusService;
use crate::testing::{MockEventFreshnessRepo, MockNetworkStatusRepo, make_network_status, ts};

// ── Happy path ───────────────────────────────────────────────────────

#[tokio::test]
async fn returns_aggregate_when_singleton_present() {
    let status = make_network_status();

    let svc = NetworkStatusService::new(
        Arc::new(MockNetworkStatusRepo::found(status.clone())),
        Arc::new(MockEventFreshnessRepo::at(ts(1_900))),
    );

    let agg = svc.get_status().await.unwrap().expect("should be Some");

    assert_eq!(agg.status.slot, status.slot);
    assert_eq!(agg.status.rpc_latency_ms, status.rpc_latency_ms);
    assert!(agg.last_event_at.is_some());
}

#[tokio::test]
async fn returns_none_when_singleton_missing() {
    // The migration seed row is absent — handler maps this to 500.
    let svc = NetworkStatusService::new(
        Arc::new(MockNetworkStatusRepo::missing()),
        // Freshness repo must NOT be called when the singleton is absent.
        // We pass `never()` which would panic if called.
        Arc::new(MockEventFreshnessRepo::never()),
    );

    // `never()` would panic on a call — if the service short-circuits
    // correctly, this completes without panic.
    let result = svc.get_status().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn freshness_is_stale_when_no_event_indexed() {
    let status = make_network_status();

    let svc = NetworkStatusService::new(
        Arc::new(MockNetworkStatusRepo::found(status)),
        Arc::new(MockEventFreshnessRepo::never()),
    );

    let agg = svc.get_status().await.unwrap().unwrap();

    assert!(agg.last_event_at.is_none());
    assert!(matches!(agg.freshness, FreshnessStatus::Stale));
}

#[tokio::test]
async fn last_event_at_is_propagated_in_aggregate() {
    let status = make_network_status();
    let event_ts = ts(1_999);

    let svc = NetworkStatusService::new(
        Arc::new(MockNetworkStatusRepo::found(status)),
        Arc::new(MockEventFreshnessRepo::at(event_ts)),
    );

    let agg = svc.get_status().await.unwrap().unwrap();
    assert_eq!(agg.last_event_at, Some(event_ts));
}

// ── Error propagation ────────────────────────────────────────────────

#[tokio::test]
async fn network_status_repo_error_propagates() {
    let svc = NetworkStatusService::new(
        Arc::new(MockNetworkStatusRepo::failing()),
        Arc::new(MockEventFreshnessRepo::never()),
    );

    assert!(matches!(
        svc.get_status().await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}

#[tokio::test]
async fn freshness_repo_error_propagates() {
    let status = make_network_status();

    let svc = NetworkStatusService::new(
        Arc::new(MockNetworkStatusRepo::found(status)),
        Arc::new(MockEventFreshnessRepo::failing()),
    );

    assert!(matches!(
        svc.get_status().await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}
