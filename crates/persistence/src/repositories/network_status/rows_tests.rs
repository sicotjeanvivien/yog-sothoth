//! Unit tests for `TryFrom<NetworkStatusRow> for NetworkStatus`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right status, and that the i64 → u64 / i32 → u32
//! narrowings surface negative values as `RepositoryError::Integrity`.

use chrono::Utc;
use yog_core::{RepositoryError, domain::NetworkStatus};

use super::NetworkStatusRow;

fn valid_row() -> NetworkStatusRow {
    NetworkStatusRow {
        slot: 300_000_000,
        rpc_latency_ms: 250,
        observed_at: Utc::now(),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_status_with_all_fields_mapped() {
    let row = valid_row();
    let observed_at = row.observed_at;

    let status = NetworkStatus::try_from(row).expect("valid row should convert");

    assert_eq!(status.slot, 300_000_000);
    assert_eq!(status.rpc_latency_ms, 250);
    assert_eq!(status.observed_at, observed_at);
}

// ── Bounds: i64 → u64 (slot) ─────────────────────────────────────────

#[test]
fn try_from_negative_slot_returns_integrity() {
    let row = NetworkStatusRow {
        slot: -1,
        ..valid_row()
    };
    let err = NetworkStatus::try_from(row).expect_err("negative slot should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Bounds: i32 → u32 (rpc_latency_ms) ───────────────────────────────

#[test]
fn try_from_negative_rpc_latency_ms_returns_integrity_with_value() {
    let row = NetworkStatusRow {
        rpc_latency_ms: -1,
        ..valid_row()
    };
    let err = NetworkStatus::try_from(row)
        .expect_err("negative rpc_latency_ms should fail u32 conversion");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid rpc_latency_ms") && msg.contains("-1"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}
