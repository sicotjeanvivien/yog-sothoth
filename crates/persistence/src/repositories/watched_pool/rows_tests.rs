//! Unit tests for `TryFrom<WatchedPoolRow> for WatchedPool`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right entry, that each fallible field has its
//! own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::Utc;
use yog_core::{
    RepositoryError,
    domain::{Protocol, WatchedPool},
};

use super::WatchedPoolRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> WatchedPoolRow {
    WatchedPoolRow {
        pool_address: VALID_POOL.into(),
        protocol: Protocol::MeteoraDammV2.as_str().to_string(),
        active: true,
        added_at: Utc::now(),
        note: Some("test pool".into()),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_watched_pool_with_all_fields_mapped() {
    let row = valid_row();
    let added_at = row.added_at;

    let watched = WatchedPool::try_from(row).expect("valid row should convert");

    assert_eq!(watched.pool_address.to_string(), VALID_POOL);
    assert_eq!(watched.protocol, Protocol::MeteoraDammV2);
    assert!(watched.active);
    assert_eq!(watched.added_at, added_at);
    assert_eq!(watched.note.as_deref(), Some("test pool"));
}

#[test]
fn try_from_inactive_with_none_note_returns_corresponding_fields() {
    // Pin the two passthrough fields that don't go through
    // validation: the active boolean (catches accidental hardcode)
    // and the note Option (catches Some/None inversion).
    let row = WatchedPoolRow {
        active: false,
        note: None,
        ..valid_row()
    };

    let watched = WatchedPool::try_from(row).expect("valid row should convert");

    assert!(!watched.active);
    assert!(watched.note.is_none());
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = WatchedPoolRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = WatchedPool::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Enum validation ──────────────────────────────────────────────────

#[test]
fn try_from_invalid_protocol_returns_integrity_with_message() {
    let row = WatchedPoolRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };
    let err = WatchedPool::try_from(row).expect_err("unknown protocol should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid protocol"),
        "expected message to mention the failure context, got: {msg}"
    );
}
