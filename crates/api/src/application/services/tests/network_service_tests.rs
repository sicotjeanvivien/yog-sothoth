//! Unit tests for `NetworkService`.

use std::sync::Arc;

use yog_core::RepositoryError;

use super::super::NetworkService;
use crate::testing::{MockNetworkStatusRepo, make_network_status};

#[tokio::test]
async fn returns_the_snapshot_when_the_singleton_is_present() {
    let status = make_network_status();
    let svc = NetworkService::new(Arc::new(MockNetworkStatusRepo::found(status.clone())));

    assert_eq!(svc.get().await.unwrap(), Some(status));
}

#[tokio::test]
async fn returns_none_when_the_singleton_is_missing() {
    let svc = NetworkService::new(Arc::new(MockNetworkStatusRepo::missing()));

    assert_eq!(svc.get().await.unwrap(), None);
}

#[tokio::test]
async fn repo_error_propagates() {
    let svc = NetworkService::new(Arc::new(MockNetworkStatusRepo::failing()));

    assert!(matches!(
        svc.get().await.unwrap_err(),
        RepositoryError::Integrity(_)
    ));
}
