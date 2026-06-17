//! Unit tests for `TryFrom<PoolAnalyticsRow> for (Pubkey, PoolAnalytics)`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right (key, value) pair, and that errors surface
//! as `RepositoryError::Integrity`.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::{RepositoryError, domain::PoolAnalytics};

use super::PoolAnalyticsRow;

const VALID_POOL: &str = "5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1";

fn valid_row() -> PoolAnalyticsRow {
    PoolAnalyticsRow {
        pool_address: VALID_POOL.into(),
        tvl_usd: Some(BigDecimal::from_str("1234.56").unwrap()),
        volume_24h_usd: Some(BigDecimal::from_str("789.01").unwrap()),
        fees_24h_usd: Some(BigDecimal::from_str("12.34").unwrap()),
        protocol_fees_24h_usd: Some(BigDecimal::from_str("2.46").unwrap()),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_pair_with_all_fields_mapped() {
    let (pool_address, analytics) =
        <(Pubkey, PoolAnalytics)>::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(pool_address.to_string(), VALID_POOL);
    assert_eq!(
        analytics.tvl_usd,
        Some(Decimal::from_str("1234.56").unwrap())
    );
    assert_eq!(
        analytics.volume_24h_usd,
        Some(Decimal::from_str("789.01").unwrap())
    );
    assert_eq!(
        analytics.fees_24h_usd,
        Some(Decimal::from_str("12.34").unwrap())
    );
    assert_eq!(
        analytics.protocol_fees_24h_usd,
        Some(Decimal::from_str("2.46").unwrap())
    );
}

#[test]
fn try_from_with_none_metrics_returns_pair_with_none_analytics() {
    // No TVL data, no recent swap activity. Maps to PoolAnalytics with
    // both fields None — never partial.
    let row = PoolAnalyticsRow {
        tvl_usd: None,
        volume_24h_usd: None,
        ..valid_row()
    };

    let (_, analytics) =
        <(Pubkey, PoolAnalytics)>::try_from(row).expect("None metrics should convert");

    assert_eq!(analytics.tvl_usd, None);
    assert_eq!(analytics.volume_24h_usd, None);
}

#[test]
fn try_from_with_only_tvl_returns_pair_with_only_tvl() {
    // TVL known but no swap in the last 24h.
    let row = PoolAnalyticsRow {
        volume_24h_usd: None,
        ..valid_row()
    };

    let (_, analytics) = <(Pubkey, PoolAnalytics)>::try_from(row).expect("should convert");

    assert!(analytics.tvl_usd.is_some());
    assert_eq!(analytics.volume_24h_usd, None);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_pool_address_returns_integrity() {
    let row = PoolAnalyticsRow {
        pool_address: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = <(Pubkey, PoolAnalytics)>::try_from(row).expect_err("invalid pubkey should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── BigDecimal → Decimal overflow ────────────────────────────────────

#[test]
fn try_from_tvl_overflowing_decimal_returns_integrity_with_field() {
    // Decimal's max is ~7.92e28; 1e30 is too large to fit.
    let huge = BigDecimal::from_str("1000000000000000000000000000000").unwrap();
    let row = PoolAnalyticsRow {
        tvl_usd: Some(huge),
        ..valid_row()
    };
    let err = <(Pubkey, PoolAnalytics)>::try_from(row)
        .expect_err("BigDecimal beyond Decimal range should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("tvl_usd"),
        "expected message to identify the failing field, got: {msg}"
    );
}
