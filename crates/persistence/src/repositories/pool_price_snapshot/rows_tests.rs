//! Unit tests for `TryFrom<PoolPriceSnapshotRow> for PoolPriceSnapshot`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid ones
//! produce the right snapshot, that each fallible field has its own
//! validation path, and that errors surface as `RepositoryError::Integrity`.
//! (The happy path against the real view is covered by the DB-backed
//! integration test in `tests/pool_price_snapshot.rs`.)

use bigdecimal::BigDecimal;
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{PoolPriceSnapshot, Protocol},
};

use super::PoolPriceSnapshotRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";
const VALID_PROTOCOL: &str = "meteora_damm_v2";

fn valid_row() -> PoolPriceSnapshotRow {
    let now = Utc::now();
    PoolPriceSnapshotRow {
        pool_address: VALID_POOL.into(),
        protocol: VALID_PROTOCOL.into(),
        // 2^64 — a realistic Q64.64 magnitude, past the u64 range.
        last_sqrt_price: "18446744073709551616".parse().unwrap(),
        last_swap_at: now,
        decimals_a: 6,
        decimals_b: 9,
        price_a_usd: "2.5".parse().unwrap(),
        price_a_fetched_at: now - Duration::seconds(10),
        price_b_usd: "100".parse().unwrap(),
        price_b_fetched_at: now - Duration::seconds(20),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_snapshot_with_all_fields_mapped() {
    let row = valid_row();
    // Three distinct timestamps — a field swap in the TryFrom would
    // surface as a mismatched assert below.
    let last_swap_at = row.last_swap_at;
    let price_a_fetched_at = row.price_a_fetched_at;
    let price_b_fetched_at = row.price_b_fetched_at;

    let snapshot = PoolPriceSnapshot::try_from(row).expect("valid row should convert");

    assert_eq!(snapshot.pool_address.to_string(), VALID_POOL);
    assert_eq!(snapshot.protocol, Protocol::MeteoraDammV2);
    assert_eq!(snapshot.sqrt_price, 1u128 << 64);
    assert_eq!(snapshot.last_swap_at, last_swap_at);
    assert_eq!(snapshot.decimals_a, 6);
    assert_eq!(snapshot.decimals_b, 9);
    assert_eq!(snapshot.price_a_usd, Decimal::new(25, 1));
    assert_eq!(snapshot.price_a_fetched_at, price_a_fetched_at);
    assert_eq!(snapshot.price_b_usd, Decimal::from(100));
    assert_eq!(snapshot.price_b_fetched_at, price_b_fetched_at);
}

// ── Pubkey / enum validation ─────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = PoolPriceSnapshotRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = PoolPriceSnapshot::try_from(row).expect_err("bad pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_unknown_protocol_returns_integrity() {
    let row = PoolPriceSnapshotRow {
        protocol: "definitely_not_a_protocol".into(),
        ..valid_row()
    };
    let err = PoolPriceSnapshot::try_from(row).expect_err("unknown protocol should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid protocol"),
        "expected message to mention the field, got: {msg}"
    );
}

// ── BigDecimal → u128 ────────────────────────────────────────────────

#[test]
fn try_from_negative_sqrt_price_returns_integrity() {
    let row = PoolPriceSnapshotRow {
        last_sqrt_price: BigDecimal::from(-1_i64),
        ..valid_row()
    };
    let err = PoolPriceSnapshot::try_from(row)
        .expect_err("negative BigDecimal should fail u128 conversion");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── SMALLINT → u8 narrowing ──────────────────────────────────────────

#[test]
fn try_from_negative_decimals_a_returns_integrity_with_field_in_message() {
    let row = PoolPriceSnapshotRow {
        decimals_a: -1,
        ..valid_row()
    };
    let err = PoolPriceSnapshot::try_from(row).expect_err("negative decimals should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("decimals_a"),
        "expected message to name the field, got: {msg}"
    );
}

#[test]
fn try_from_out_of_range_decimals_b_returns_integrity() {
    let row = PoolPriceSnapshotRow {
        decimals_b: 300,
        ..valid_row()
    };
    let err = PoolPriceSnapshot::try_from(row).expect_err(">255 decimals should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── BigDecimal → Decimal (price magnitude) ───────────────────────────

#[test]
fn try_from_price_beyond_decimal_range_returns_integrity() {
    // NUMERIC can carry magnitudes rust_decimal cannot (max ~7.9e28).
    let row = PoolPriceSnapshotRow {
        price_a_usd: "1e40".parse().unwrap(),
        ..valid_row()
    };
    let err = PoolPriceSnapshot::try_from(row).expect_err("overflowing price should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
