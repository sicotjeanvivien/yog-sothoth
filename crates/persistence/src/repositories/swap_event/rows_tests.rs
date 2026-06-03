//! Unit tests for `TryFrom<SwapEventRow> for SwapEvent`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right `SwapEvent`, that each fallible field has
//! its own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use solana_signature::Signature;
use sqlx::types::BigDecimal;
use yog_core::{
    RepositoryError,
    domain::{Protocol, SwapEvent, TradeDirection},
};

use super::SwapEventRow;

// Distinct valid base58-encoded Pubkeys so any field swap in the
// `TryFrom` impl shows up immediately in the happy path test.
const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_TOKEN_A: &str = "So11111111111111111111111111111111111111112";
const VALID_TOKEN_B: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn valid_row() -> SwapEventRow {
    SwapEventRow {
        pool_address: VALID_POOL.into(),
        protocol: Protocol::MeteoraDammV2.as_str().to_string(),
        signature: sig(1).to_string(),
        timestamp: Utc::now(),
        token_a_mint: VALID_TOKEN_A.into(),
        token_b_mint: VALID_TOKEN_B.into(),
        trade_direction: TradeDirection::AtoB.as_str().to_string(),
        amount_a: 1_000_000,
        amount_b: 2_000_000,
        reserve_a_after: 10_000_000,
        reserve_b_after: 20_000_000,
        next_sqrt_price: BigDecimal::from(42_000_000_u64),
        claiming_fee: 100,
        protocol_fee: 200,
        compounding_fee: 300,
        referral_fee: 400,
        fee_token_is_a: true,
    }
}

fn sig(seed: u8) -> Signature {
    Signature::from([seed; 64])
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_event_with_all_fields_mapped() {
    let event = SwapEvent::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(event.pool_address.to_string(), VALID_POOL);
    assert_eq!(event.protocol, Protocol::MeteoraDammV2);
    assert_eq!(event.token_a_mint.to_string(), VALID_TOKEN_A);
    assert_eq!(event.token_b_mint.to_string(), VALID_TOKEN_B);
    assert_eq!(event.trade_direction, TradeDirection::AtoB);
    assert_eq!(event.amount_a, 1_000_000);
    assert_eq!(event.amount_b, 2_000_000);
    assert_eq!(event.reserve_a_after, 10_000_000);
    assert_eq!(event.reserve_b_after, 20_000_000);
    assert_eq!(event.next_sqrt_price, 42_000_000_u128);
    assert_eq!(event.claiming_fee, 100);
    assert_eq!(event.protocol_fee, 200);
    assert_eq!(event.compounding_fee, 300);
    assert_eq!(event.referral_fee, 400);
    assert!(event.fee_token_is_a);
}

#[test]
fn try_from_preserves_signature_and_timestamp() {
    let signature = sig(1).to_string();
    let timestamp = Utc::now() + Duration::seconds(123);
    let row = SwapEventRow {
        signature: signature.clone(),
        timestamp,
        ..valid_row()
    };

    let event = SwapEvent::try_from(row).expect("valid row should convert");

    assert_eq!(event.signature.to_string(), signature);
    assert_eq!(event.timestamp, timestamp);
}

#[test]
fn try_from_preserves_fee_token_is_a_false() {
    // Happy path pins `true`; also pin `false` to catch an accidental
    // hardcode in the TryFrom impl.
    let row = SwapEventRow {
        fee_token_is_a: false,
        ..valid_row()
    };

    let event = SwapEvent::try_from(row).expect("valid row should convert");

    assert!(!event.fee_token_is_a);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = SwapEventRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_token_a_mint_returns_integrity() {
    let row = SwapEventRow {
        token_a_mint: "garbage".into(),
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("invalid token_a should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_token_b_mint_returns_integrity() {
    let row = SwapEventRow {
        token_b_mint: "garbage".into(),
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("invalid token_b should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Enum validation ──────────────────────────────────────────────────

#[test]
fn try_from_invalid_protocol_returns_integrity_with_message() {
    let row = SwapEventRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("unknown protocol should fail");
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
fn try_from_invalid_trade_direction_returns_integrity_with_value_in_message() {
    let row = SwapEventRow {
        trade_direction: "sideways".into(),
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("unknown trade_direction should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid trade_direction") && msg.contains("sideways"),
        "expected message to mention both the field and the bad value, got: {msg}"
    );
}

// ── Numeric conversion: i64 → u64 (one test per field) ───────────────

#[test]
fn try_from_negative_amount_a_returns_integrity() {
    let row = SwapEventRow {
        amount_a: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_amount_b_returns_integrity() {
    let row = SwapEventRow {
        amount_b: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_a_after_returns_integrity() {
    let row = SwapEventRow {
        reserve_a_after: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_reserve_b_after_returns_integrity() {
    let row = SwapEventRow {
        reserve_b_after: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_claiming_fee_returns_integrity() {
    let row = SwapEventRow {
        claiming_fee: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_protocol_fee_returns_integrity() {
    let row = SwapEventRow {
        protocol_fee: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_compounding_fee_returns_integrity() {
    let row = SwapEventRow {
        compounding_fee: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_negative_referral_fee_returns_integrity() {
    let row = SwapEventRow {
        referral_fee: -1,
        ..valid_row()
    };
    let err = SwapEvent::try_from(row).expect_err("negative i64 should fail u64 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── Numeric conversion: BigDecimal → u128 ────────────────────────────

#[test]
fn try_from_negative_next_sqrt_price_returns_integrity() {
    let row = SwapEventRow {
        next_sqrt_price: BigDecimal::from(-1_i64),
        ..valid_row()
    };
    let err =
        SwapEvent::try_from(row).expect_err("negative BigDecimal should fail u128 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
