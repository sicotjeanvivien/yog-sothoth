//! Unit tests for `AnnouncementService`.

use std::sync::Arc;

use yog_core::RepositoryError;

use super::super::AnnouncementService;
use crate::testing::{MockAnnouncementRepo, make_announcement};

// ── Happy path ───────────────────────────────────────────────────────

#[tokio::test]
async fn returns_repository_order_untouched() {
    let announcements = vec![make_announcement(1), make_announcement(2)];

    let svc = AnnouncementService::new(Arc::new(MockAnnouncementRepo::active(
        announcements.clone(),
    )));

    let result = svc.list_active().await.unwrap();

    assert_eq!(result, announcements);
}

#[tokio::test]
async fn empty_active_window_is_not_an_error() {
    let svc = AnnouncementService::new(Arc::new(MockAnnouncementRepo::active(Vec::new())));

    let result = svc.list_active().await.unwrap();

    assert!(result.is_empty());
}

// ── Errors ───────────────────────────────────────────────────────────

#[tokio::test]
async fn repo_error_propagates() {
    let svc = AnnouncementService::new(Arc::new(MockAnnouncementRepo::failing()));

    let err = svc.list_active().await.expect_err("should propagate");

    assert!(matches!(err, RepositoryError::Integrity(_)));
}
