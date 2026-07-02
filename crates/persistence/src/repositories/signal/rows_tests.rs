//! Unit tests for `TryFrom<SignalRow> for SignalRecord`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid ones
//! produce the right record, that each fallible field has its own
//! validation path, and that errors surface as `RepositoryError::Integrity`.
//! (The read against the real table is covered by the DB-backed
//! integration test in `tests/signal_list.rs`.)

use bigdecimal::BigDecimal;
use chrono::Utc;
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{Protocol, Severity, SignalRecord},
};

use super::SignalRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> SignalRow {
    SignalRow {
        id: 42,
        detector: "flow_imbalance".into(),
        protocol: "meteora_damm_v2".into(),
        pool_address: VALID_POOL.into(),
        severity: "warning".into(),
        value: "0.75".parse().unwrap(),
        threshold: Some("0.6".parse().unwrap()),
        message: Some("directional flow imbalance 0.75".into()),
        triggered_at: Utc::now(),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_record_with_all_fields_mapped() {
    let row = valid_row();
    let triggered_at = row.triggered_at;

    let record = SignalRecord::try_from(row).expect("valid row should convert");

    assert_eq!(record.id, 42);
    assert_eq!(record.signal.detector, "flow_imbalance");
    assert_eq!(record.signal.protocol, Protocol::MeteoraDammV2);
    assert_eq!(record.signal.pool_address.to_string(), VALID_POOL);
    assert_eq!(record.signal.severity, Severity::Warning);
    assert_eq!(record.signal.value, Decimal::new(75, 2));
    assert_eq!(record.signal.threshold, Some(Decimal::new(6, 1)));
    assert_eq!(
        record.signal.message.as_deref(),
        Some("directional flow imbalance 0.75")
    );
    assert_eq!(record.signal.triggered_at, triggered_at);
}

#[test]
fn try_from_none_optionals_stay_none() {
    let row = SignalRow {
        threshold: None,
        message: None,
        ..valid_row()
    };

    let record = SignalRecord::try_from(row).expect("None optionals should convert");

    assert_eq!(record.signal.threshold, None);
    assert_eq!(record.signal.message, None);
}

// ── Validation paths ─────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = SignalRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = SignalRecord::try_from(row).expect_err("bad pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_unknown_protocol_returns_integrity() {
    let row = SignalRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };
    let err = SignalRecord::try_from(row).expect_err("unknown protocol should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_unknown_severity_returns_integrity_with_value_in_message() {
    let row = SignalRow {
        severity: "panic".into(),
        ..valid_row()
    };
    let err = SignalRecord::try_from(row).expect_err("unknown severity should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("severity") && msg.contains("panic"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}

#[test]
fn try_from_value_beyond_decimal_range_returns_integrity() {
    // NUMERIC can carry magnitudes rust_decimal cannot (max ~7.9e28).
    let row = SignalRow {
        value: "1e40".parse::<BigDecimal>().unwrap(),
        ..valid_row()
    };
    let err = SignalRecord::try_from(row).expect_err("overflowing value should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
