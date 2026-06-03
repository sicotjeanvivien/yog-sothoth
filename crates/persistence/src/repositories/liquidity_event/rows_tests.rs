//! Unit tests for `TryFrom<LiquidityEventRow> for LiquidityEvent`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right `LiquidityEvent`, that each fallible field
//! has its own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use sqlx::types::BigDecimal;
use yog_core::{
    RepositoryError,
    domain::{LiquidityEvent, LiquidityEventKind, Protocol},
};

use super::LiquidityEventRow;

// Distinct valid base58-encoded Pubkeys so any field swap in the
// `TryFrom` impl shows up immediately in the happy path test.
const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_TOKEN_A: &str = "So11111111111111111111111111111111111111112";
const VALID_TOKEN_B: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const VALID_POSITION: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
const VALID_OWNER: &str = "11111111111111111111111111111111";

fn valid_row() -> LiquidityEventRow {
    LiquidityEventRow {
        pool_address: VALID_POOL.into(),
        protocol: Protocol::MeteoraDammV2.as_str().to_string(),
        signature: "sig_5xY3Z".into(),
        timestamp: Utc::now(),
        token_a_mint: VALID_TOKEN_A.into(),
        token_b_mint: VALID_TOKEN_B.into(),
        liquidity_event_kind: LiquidityEventKind::Add.as_str().to_string(),
        amount_a: 1_000_000,
        amount_b: 2_000_000,
        liquidity_delta: BigDecimal::from(42_000_000_u64),
        reserve_a_after: 10_000_000,
        reserve_b_after: 20_000_000,
        position: VALID_POSITION.into(),
        owner: VALID_OWNER.into(),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_event_with_all_fields_mapped() {
    let event = LiquidityEvent::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(event.pool_address.to_string(), VALID_POOL);
    assert_eq!(event.protocol, Protocol::MeteoraDammV2);
    assert_eq!(event.token_a_mint.to_string(), VALID_TOKEN_A);
    assert_eq!(event.token_b_mint.to_string(), VALID_TOKEN_B);
    assert_eq!(event.position.to_string(), VALID_POSITION);
    assert_eq!(event.owner.to_string(), VALID_OWNER);
    assert_eq!(event.liquidity_event_kind, LiquidityEventKind::Add);
    assert_eq!(event.amount_a, 1_000_000);
    assert_eq!(event.amount_b, 2_000_000);
    assert_eq!(event.liquidity_delta, 42_000_000_u128);
    assert_eq!(event.reserve_a_after, 10_000_000);
    assert_eq!(event.reserve_b_after, 20_000_000);
}

#[test]
fn try_from_preserves_signature_and_timestamp() {
    let signature = "abc123def456".to_string();
    let timestamp = Utc::now() + Duration::seconds(123);
    let row = LiquidityEventRow {
        signature: signature.clone(),
        timestamp,
        ..valid_row()
    };

    let event = LiquidityEvent::try_from(row).expect("valid row should convert");

    assert_eq!(event.signature, signature);
    assert_eq!(event.timestamp, timestamp);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = LiquidityEventRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_token_a_mint_returns_integrity() {
    let row = LiquidityEventRow {
        token_a_mint: "garbage".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("invalid token_a should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_token_b_mint_returns_integrity() {
    let row = LiquidityEventRow {
        token_b_mint: "garbage".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("invalid token_b should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_position_returns_integrity() {
    let row = LiquidityEventRow {
        position: "garbage".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("invalid position should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_owner_returns_integrity() {
    let row = LiquidityEventRow {
        owner: "garbage".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("invalid owner should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Enum validation ──────────────────────────────────────────────────

#[test]
fn try_from_invalid_protocol_returns_integrity_with_message() {
    let row = LiquidityEventRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("unknown protocol should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid protocol"),
        "expected message to mention the failure context, got: {msg}"
    );
}

#[test]
fn try_from_invalid_liquidity_event_kind_returns_integrity() {
    let row = LiquidityEventRow {
        liquidity_event_kind: "definitely_not_a_kind".into(),
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("unknown kind should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Numeric conversion (i64 → u64, BigDecimal → u128) ────────────────

#[test]
fn try_from_negative_amount_a_returns_integrity() {
    let row = LiquidityEventRow {
        amount_a: -1,
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_amount_b_returns_integrity() {
    let row = LiquidityEventRow {
        amount_b: -1,
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_a_after_returns_integrity() {
    let row = LiquidityEventRow {
        reserve_a_after: -1,
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_b_after_returns_integrity() {
    let row = LiquidityEventRow {
        reserve_b_after: -1,
        ..valid_row()
    };
    let err = LiquidityEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_liquidity_delta_returns_integrity() {
    let row = LiquidityEventRow {
        liquidity_delta: BigDecimal::from(-1_i64),
        ..valid_row()
    };
    let err =
        LiquidityEvent::try_from(row).expect_err("negative BigDecimal should fail u128 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
