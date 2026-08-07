//! Unit tests for `TryFrom<PoolSwapFlowRow> for PoolSwapFlow`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid ones
//! map each direction to its own field and that errors surface as
//! `RepositoryError::Integrity`. (The happy path against the real view is
//! covered by the DB-backed integration test in `tests/swap_flow.rs`.)

use rust_decimal::Decimal;
use yog_core::{RepositoryError, domain::PoolSwapFlow};

use super::PoolSwapFlowRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> PoolSwapFlowRow {
    PoolSwapFlowRow {
        pool_address: VALID_POOL.into(),
        // Two distinct values — a direction swap in the TryFrom would
        // surface as a mismatched assert.
        volume_a_to_b_usd: Some("1234.5".parse().unwrap()),
        volume_b_to_a_usd: Some("678.9".parse().unwrap()),
    }
}

#[test]
fn try_from_valid_row_maps_each_direction_to_its_field() {
    let flow = PoolSwapFlow::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(flow.pool_address.to_string(), VALID_POOL);
    assert_eq!(flow.volume_a_to_b_usd, Some(Decimal::new(12_345, 1)));
    assert_eq!(flow.volume_b_to_a_usd, Some(Decimal::new(6_789, 1)));
}

#[test]
fn try_from_null_volumes_maps_to_none_rather_than_zero() {
    // The query returns NULL for a window that was not entirely valuable. It
    // must survive the conversion AS an absence: mapping it to `Decimal::ZERO`
    // here would re-introduce, one layer lower, the exact collapse the
    // repository stopped doing (`.project` ticket 08).
    let row = PoolSwapFlowRow {
        pool_address: VALID_POOL.into(),
        volume_a_to_b_usd: None,
        volume_b_to_a_usd: None,
    };
    let flow = PoolSwapFlow::try_from(row).expect("a NULL volume is valid, not an error");

    assert_eq!(flow.volume_a_to_b_usd, None);
    assert_eq!(flow.volume_b_to_a_usd, None);
}

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = PoolSwapFlowRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = PoolSwapFlow::try_from(row).expect_err("bad pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

#[test]
fn try_from_volume_beyond_decimal_range_returns_integrity() {
    // NUMERIC can carry magnitudes rust_decimal cannot (max ~7.9e28).
    let row = PoolSwapFlowRow {
        volume_a_to_b_usd: Some("1e40".parse().unwrap()),
        ..valid_row()
    };
    let err = PoolSwapFlow::try_from(row).expect_err("overflowing volume should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}
