//! Unit tests for `TryFrom<AnnouncementRow> for Announcement`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid ones
//! map every field, and that a TEXT tag outside the CHECK-constrained
//! sets (schema/code drift) surfaces as `RepositoryError::Integrity`.

use chrono::Utc;
use yog_core::{
    RepositoryError,
    domain::{Announcement, AnnouncementKind, AnnouncementSeverity},
};

use super::AnnouncementRow;

fn valid_row() -> AnnouncementRow {
    AnnouncementRow {
        id: 1,
        kind: "release".to_string(),
        severity: "info".to_string(),
        message: "v0.1.1 is live".to_string(),
        link_url: Some("/changelog#v0.1.1".to_string()),
        starts_at: Utc::now(),
        ends_at: None,
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_maps_all_fields() {
    let row = valid_row();
    let starts_at = row.starts_at;

    let announcement = Announcement::try_from(row).expect("valid row should convert");

    assert_eq!(announcement.id, 1);
    assert_eq!(announcement.kind, AnnouncementKind::Release);
    assert_eq!(announcement.severity, AnnouncementSeverity::Info);
    assert_eq!(announcement.message, "v0.1.1 is live");
    assert_eq!(announcement.link_url.as_deref(), Some("/changelog#v0.1.1"));
    assert_eq!(announcement.starts_at, starts_at);
    assert_eq!(announcement.ends_at, None);
}

// ── Tag drift → Integrity ────────────────────────────────────────────

#[test]
fn try_from_unknown_kind_returns_integrity_with_value() {
    let row = AnnouncementRow {
        kind: "party".to_string(),
        ..valid_row()
    };
    let err = Announcement::try_from(row).expect_err("unknown kind should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid kind") && msg.contains("party"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}

#[test]
fn try_from_unknown_severity_returns_integrity_with_value() {
    let row = AnnouncementRow {
        severity: "loud".to_string(),
        ..valid_row()
    };
    let err = Announcement::try_from(row).expect_err("unknown severity should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid severity") && msg.contains("loud"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}
