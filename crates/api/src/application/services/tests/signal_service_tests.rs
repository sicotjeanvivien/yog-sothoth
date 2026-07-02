//! Unit tests for `SignalService`.

use std::sync::Arc;

use yog_core::{PageDirection, RepositoryError};

use super::super::{SignalListParams, SignalService};
use crate::testing::{MockSignalRepo, make_signal_page, make_signal_record, pk};

fn default_params() -> SignalListParams {
    SignalListParams {
        severity: None,
        cursor: None,
        direction: PageDirection::Next,
        position: None,
        limit: 50,
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[tokio::test]
async fn returns_page_items_from_repo() {
    let record = make_signal_record(42, pk(1));
    let page = make_signal_page(vec![record.clone()], true);

    let svc = SignalService::new(Arc::new(MockSignalRepo::with_page(page)));

    let result = svc.list_signals(default_params()).await.unwrap();

    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].id, 42);
    assert_eq!(result.items[0].signal.pool_address, pk(1));
}

#[tokio::test]
async fn empty_page_is_not_an_error() {
    let svc = SignalService::new(Arc::new(MockSignalRepo::empty()));

    let result = svc.list_signals(default_params()).await.unwrap();

    assert!(result.items.is_empty());
    assert!(result.is_first);
    assert!(result.is_last);
}

// ── Error propagation ────────────────────────────────────────────────

#[tokio::test]
async fn repository_error_bubbles_up() {
    let svc = SignalService::new(Arc::new(MockSignalRepo::failing()));

    let err = svc.list_signals(default_params()).await.unwrap_err();

    assert!(matches!(err, RepositoryError::Integrity(_)));
}
