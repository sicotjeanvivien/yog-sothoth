//! Unit tests for `TryFrom<GlobalAnalyticsRow> for GlobalAnalytics`.
//!
//! Pure parser tests, no DB. Build rows by hand and assert that valid
//! ones map field-for-field, that absent USD aggregates stay `None`
//! (partial-coverage contract), and that a BigDecimal beyond `Decimal`
//! range surfaces as `RepositoryError::Integrity` naming the field.

use std::str::FromStr;

use bigdecimal::BigDecimal;
use rust_decimal::Decimal;
use yog_core::{RepositoryError, domain::GlobalAnalytics};

use super::GlobalAnalyticsRow;

fn valid_row() -> GlobalAnalyticsRow {
    GlobalAnalyticsRow {
        total_tvl_usd: Some(BigDecimal::from_str("10427935.81").unwrap()),
        pools_priced: 348,
        volume_24h_usd: Some(BigDecimal::from_str("508193.05").unwrap()),
        fees_24h_usd: Some(BigDecimal::from_str("391.03").unwrap()),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_maps_all_fields() {
    let analytics = GlobalAnalytics::try_from(valid_row()).expect("valid row should convert");

    assert_eq!(
        analytics.total_tvl_usd,
        Some(Decimal::from_str("10427935.81").unwrap())
    );
    assert_eq!(analytics.pools_priced, 348);
    assert_eq!(
        analytics.volume_24h_usd,
        Some(Decimal::from_str("508193.05").unwrap())
    );
    assert_eq!(
        analytics.fees_24h_usd,
        Some(Decimal::from_str("391.03").unwrap())
    );
}

// ── Empty universe / partial coverage ────────────────────────────────

#[test]
fn try_from_all_usd_none_keeps_none_and_count() {
    // No priceable pool and no recent activity: every SUM is NULL, but the
    // count is a non-null BIGINT (here 0). USD fields stay None, never 0.
    let row = GlobalAnalyticsRow {
        total_tvl_usd: None,
        pools_priced: 0,
        volume_24h_usd: None,
        fees_24h_usd: None,
    };

    let analytics = GlobalAnalytics::try_from(row).expect("all-None metrics should convert");

    assert_eq!(analytics.total_tvl_usd, None);
    assert_eq!(analytics.volume_24h_usd, None);
    assert_eq!(analytics.fees_24h_usd, None);
    assert_eq!(analytics.pools_priced, 0);
}

#[test]
fn try_from_tvl_present_volume_absent() {
    // Pools priced (TVL known) but no swap in the last 24h.
    let row = GlobalAnalyticsRow {
        volume_24h_usd: None,
        fees_24h_usd: None,
        ..valid_row()
    };

    let analytics = GlobalAnalytics::try_from(row).expect("should convert");

    assert!(analytics.total_tvl_usd.is_some());
    assert_eq!(analytics.pools_priced, 348);
    assert_eq!(analytics.volume_24h_usd, None);
    assert_eq!(analytics.fees_24h_usd, None);
}

// ── BigDecimal → Decimal overflow ────────────────────────────────────

#[test]
fn try_from_overflowing_volume_returns_integrity_with_field() {
    // Decimal's max is ~7.92e28; 1e30 is too large to fit.
    let huge = BigDecimal::from_str("1000000000000000000000000000000").unwrap();
    let row = GlobalAnalyticsRow {
        volume_24h_usd: Some(huge),
        ..valid_row()
    };

    let err =
        GlobalAnalytics::try_from(row).expect_err("BigDecimal beyond Decimal range should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("volume_24h_usd"),
        "expected message to identify the failing field, got: {msg}"
    );
}
