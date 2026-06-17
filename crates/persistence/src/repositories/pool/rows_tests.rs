//! Unit tests for `TryFrom<PoolRow> for Pool`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right `Pool`, that each individual field has its
//! own validation path, and that errors surface as
//! `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{Pool, Protocol},
};

use super::PoolRow;

// Three distinct, valid base58-encoded Solana pubkeys. Using distinct
// values for pool / token_a / token_b is intentional: it catches any
// future field swap in the `TryFrom` impl.
const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_TOKEN_A: &str = "So11111111111111111111111111111111111111112";
const VALID_TOKEN_B: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

fn valid_row() -> PoolRow {
    let now = Utc::now();
    PoolRow {
        pool_address: VALID_POOL.into(),
        protocol: Protocol::MeteoraDammV2.as_str().to_string(),
        token_a_mint: Some(VALID_TOKEN_A.into()),
        token_b_mint: Some(VALID_TOKEN_B.into()),
        fee_bps: Some(Decimal::new(25, 0)),
        protocol_fee_percent: Some(20),
        partner_fee_percent: Some(0),
        referral_fee_percent: Some(20),
        first_seen_at: now,
        last_seen_at: now,
    }
}

#[test]
fn try_from_valid_row_returns_pool_with_all_fields_mapped() {
    let pool = Pool::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(pool.pool_address.to_string(), VALID_POOL);
    assert_eq!(pool.protocol, Protocol::MeteoraDammV2);
    assert_eq!(pool.token_a_mint.unwrap().to_string(), VALID_TOKEN_A);
    assert_eq!(pool.token_b_mint.unwrap().to_string(), VALID_TOKEN_B);
    assert_eq!(pool.fee_bps, Some(Decimal::new(25, 0)));
    assert_eq!(pool.protocol_fee_percent, Some(20));
    assert_eq!(pool.partner_fee_percent, Some(0));
    assert_eq!(pool.referral_fee_percent, Some(20));
}

#[test]
fn try_from_null_fee_percents_maps_to_none() {
    let row = PoolRow {
        protocol_fee_percent: None,
        partner_fee_percent: None,
        referral_fee_percent: None,
        ..valid_row()
    };
    let pool = Pool::try_from(row).expect("null percents should convert");
    assert!(pool.protocol_fee_percent.is_none());
    assert!(pool.partner_fee_percent.is_none());
    assert!(pool.referral_fee_percent.is_none());
}

#[test]
fn try_from_out_of_range_percent_returns_integrity() {
    let row = PoolRow {
        protocol_fee_percent: Some(-1),
        ..valid_row()
    };
    let err = Pool::try_from(row).expect_err("negative percent should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_null_mints_maps_to_none() {
    let row = PoolRow {
        token_a_mint: None,
        token_b_mint: None,
        ..valid_row()
    };
    let pool = Pool::try_from(row).expect("null mints should convert");
    assert!(pool.token_a_mint.is_none());
    assert!(pool.token_b_mint.is_none());
}

#[test]
fn try_from_null_fee_bps_maps_to_none() {
    let row = PoolRow {
        fee_bps: None,
        ..valid_row()
    };
    let pool = Pool::try_from(row).expect("null fee_bps should convert");
    assert!(pool.fee_bps.is_none());
}

#[test]
fn try_from_preserves_timestamps_in_correct_fields() {
    // Two distinct timestamps so a field swap would be caught.
    let first = Utc::now();
    let last = first + Duration::seconds(42);
    let row = PoolRow {
        first_seen_at: first,
        last_seen_at: last,
        ..valid_row()
    };

    let pool = Pool::try_from(row).expect("valid row should convert");

    assert_eq!(pool.first_seen_at, first);
    assert_eq!(pool.last_seen_at, last);
}

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = PoolRow {
        pool_address: "not-a-real-pubkey".into(),
        ..valid_row()
    };

    let err = Pool::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_protocol_returns_integrity_with_message() {
    let row = PoolRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };

    let err = Pool::try_from(row).expect_err("unknown protocol should fail");
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
fn try_from_invalid_token_a_mint_returns_integrity() {
    let row = PoolRow {
        token_a_mint: Some("garbage".into()),
        ..valid_row()
    };

    let err = Pool::try_from(row).expect_err("invalid token_a should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_invalid_token_b_mint_returns_integrity() {
    let row = PoolRow {
        token_b_mint: Some("garbage".into()),
        ..valid_row()
    };

    let err = Pool::try_from(row).expect_err("invalid token_b should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
