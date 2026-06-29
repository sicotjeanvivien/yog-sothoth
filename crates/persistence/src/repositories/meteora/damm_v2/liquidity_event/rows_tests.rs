//! Unit tests for `TryFrom<MeteoraDammV2LiquidityEventRow> for
//! MeteoraDammV2LiquidityEventValued`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right event + `value_usd`, that each fallible field
//! has its own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use solana_signature::Signature;
use sqlx::types::BigDecimal;
use yog_core::{
    RepositoryError,
    domain::{MeteoraDammV2LiquidityEventKind, MeteoraDammV2LiquidityEventValued},
};

use super::MeteoraDammV2LiquidityEventRow;

// Distinct valid base58-encoded Pubkeys so any field swap in the
// `TryFrom` impl shows up immediately in the happy path test.
const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_POSITION: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const VALID_OWNER: &str = "11111111111111111111111111111111";

fn valid_row() -> MeteoraDammV2LiquidityEventRow {
    MeteoraDammV2LiquidityEventRow {
        pool_address: VALID_POOL.into(),
        signature: sig(1).to_string(),
        timestamp: Utc::now(),
        liquidity_event_kind: MeteoraDammV2LiquidityEventKind::Add.as_str().to_string(),
        amount_a: 1_000_000,
        amount_b: 2_000_000,
        liquidity_delta: BigDecimal::from(42_000_000_u64),
        reserve_a_after: 10_000_000,
        reserve_b_after: 20_000_000,
        position: VALID_POSITION.into(),
        owner: VALID_OWNER.into(),
        value_usd: Some(BigDecimal::from(455_u64)),
    }
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_event_with_all_fields_mapped() {
    let valued =
        MeteoraDammV2LiquidityEventValued::try_from(valid_row()).expect("valid row should convert");
    let event = valued.event;

    assert_eq!(event.pool_address.to_string(), VALID_POOL);
    assert_eq!(event.position.to_string(), VALID_POSITION);
    assert_eq!(event.owner.to_string(), VALID_OWNER);
    assert_eq!(
        event.liquidity_event_kind,
        MeteoraDammV2LiquidityEventKind::Add
    );
    assert_eq!(event.amount_a, 1_000_000);
    assert_eq!(event.amount_b, 2_000_000);
    assert_eq!(event.liquidity_delta, 42_000_000_u128);
    assert_eq!(event.reserve_a_after, 10_000_000);
    assert_eq!(event.reserve_b_after, 20_000_000);
    assert_eq!(valued.value_usd, Some(Decimal::from(455)));
}

#[test]
fn try_from_null_value_usd_maps_to_none() {
    let row = MeteoraDammV2LiquidityEventRow {
        value_usd: None,
        ..valid_row()
    };
    let valued =
        MeteoraDammV2LiquidityEventValued::try_from(row).expect("valid row should convert");
    assert_eq!(valued.value_usd, None);
}

#[test]
fn try_from_preserves_signature_and_timestamp() {
    let signature = sig(1).to_string();
    let timestamp = Utc::now() + Duration::seconds(123);
    let row = MeteoraDammV2LiquidityEventRow {
        signature: signature.clone(),
        timestamp,
        ..valid_row()
    };

    let valued =
        MeteoraDammV2LiquidityEventValued::try_from(row).expect("valid row should convert");

    assert_eq!(valued.event.signature.to_string(), signature);
    assert_eq!(valued.event.timestamp, timestamp);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err =
        MeteoraDammV2LiquidityEventValued::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_position_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        position: "garbage".into(),
        ..valid_row()
    };
    let err =
        MeteoraDammV2LiquidityEventValued::try_from(row).expect_err("invalid position should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_owner_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        owner: "garbage".into(),
        ..valid_row()
    };
    let err =
        MeteoraDammV2LiquidityEventValued::try_from(row).expect_err("invalid owner should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Enum validation ──────────────────────────────────────────────────

#[test]
fn try_from_invalid_liquidity_event_kind_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        liquidity_event_kind: "definitely_not_a_kind".into(),
        ..valid_row()
    };
    let err =
        MeteoraDammV2LiquidityEventValued::try_from(row).expect_err("unknown kind should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Numeric conversion (i64 → u64, BigDecimal → u128) ────────────────

#[test]
fn try_from_negative_amount_a_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        amount_a: -1,
        ..valid_row()
    };
    let err = MeteoraDammV2LiquidityEventValued::try_from(row)
        .expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_amount_b_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        amount_b: -1,
        ..valid_row()
    };
    let err = MeteoraDammV2LiquidityEventValued::try_from(row)
        .expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_a_after_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        reserve_a_after: -1,
        ..valid_row()
    };
    let err = MeteoraDammV2LiquidityEventValued::try_from(row)
        .expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_b_after_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        reserve_b_after: -1,
        ..valid_row()
    };
    let err = MeteoraDammV2LiquidityEventValued::try_from(row)
        .expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_liquidity_delta_returns_integrity() {
    let row = MeteoraDammV2LiquidityEventRow {
        liquidity_delta: BigDecimal::from(-1_i64),
        ..valid_row()
    };
    let err = MeteoraDammV2LiquidityEventValued::try_from(row)
        .expect_err("negative BigDecimal should fail u128 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
