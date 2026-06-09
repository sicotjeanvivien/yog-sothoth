//! Unit tests for `TryFrom<MeteoraDammV2ClaimPositionFeeEventRow> for ClaimPositionFeeEvent`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right event, that each fallible field has its
//! own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use solana_signature::Signature;
use yog_core::{RepositoryError, domain::MeteoraDammV2ClaimPositionFeeEvent};

use super::MeteoraDammV2ClaimPositionFeeEventRow;

// Distinct valid base58 Pubkeys so a field swap in the TryFrom shows
// up immediately in the happy path test.
const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_POSITION: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const VALID_OWNER: &str = "11111111111111111111111111111111";

/// Build a deterministic, valid base58 signature for the Row's String
/// column. Symmetric to `sig_for_pool` in the api crate's testing
/// fixtures — same logic, different return type (String here, since
/// the Row stores the wire form).
fn valid_sig(seed: u8) -> String {
    Signature::from([seed; 64]).to_string()
}

fn valid_row() -> MeteoraDammV2ClaimPositionFeeEventRow {
    MeteoraDammV2ClaimPositionFeeEventRow {
        pool_address: VALID_POOL.into(),
        signature: valid_sig(1),
        timestamp: Utc::now(),
        position: VALID_POSITION.into(),
        owner: VALID_OWNER.into(),
        fee_a_claimed: 1_000_000,
        fee_b_claimed: 2_000_000,
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_event_with_all_fields_mapped() {
    let event = MeteoraDammV2ClaimPositionFeeEvent::try_from(valid_row())
        .expect("valid row should convert");

    assert_eq!(event.pool_address.to_string(), VALID_POOL);
    assert_eq!(event.signature, Signature::from([1u8; 64]));
    assert_eq!(event.position.to_string(), VALID_POSITION);
    assert_eq!(event.owner.to_string(), VALID_OWNER);
    assert_eq!(event.fee_a_claimed, 1_000_000);
    assert_eq!(event.fee_b_claimed, 2_000_000);
}

#[test]
fn try_from_preserves_signature_and_timestamp() {
    // Distinct values so a field swap would surface as a mismatch.
    let expected_sig = Signature::from([42u8; 64]);
    let timestamp = Utc::now() + Duration::seconds(123);
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        signature: expected_sig.to_string(),
        timestamp,
        ..valid_row()
    };

    let event =
        MeteoraDammV2ClaimPositionFeeEvent::try_from(row).expect("valid row should convert");

    assert_eq!(event.signature, expected_sig);
    assert_eq!(event.timestamp, timestamp);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err =
        MeteoraDammV2ClaimPositionFeeEvent::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_position_returns_integrity() {
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        position: "garbage".into(),
        ..valid_row()
    };
    let err = MeteoraDammV2ClaimPositionFeeEvent::try_from(row)
        .expect_err("invalid position should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_owner_returns_integrity() {
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        owner: "garbage".into(),
        ..valid_row()
    };
    let err =
        MeteoraDammV2ClaimPositionFeeEvent::try_from(row).expect_err("invalid owner should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Signature validation ─────────────────────────────────────────────

#[test]
fn try_from_invalid_signature_returns_integrity() {
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        signature: "not-a-real-signature".into(),
        ..valid_row()
    };
    let err = MeteoraDammV2ClaimPositionFeeEvent::try_from(row)
        .expect_err("invalid signature should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Numeric conversion (i64 → u64) ───────────────────────────────────

#[test]
fn try_from_negative_fee_a_claimed_returns_integrity() {
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        fee_a_claimed: -1,
        ..valid_row()
    };
    let err = MeteoraDammV2ClaimPositionFeeEvent::try_from(row)
        .expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_fee_b_claimed_returns_integrity() {
    let row = MeteoraDammV2ClaimPositionFeeEventRow {
        fee_b_claimed: -1,
        ..valid_row()
    };
    let err = MeteoraDammV2ClaimPositionFeeEvent::try_from(row)
        .expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
