//! Unit tests for `TryFrom<TokenPriceRow> for TokenPrice`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right price, that each fallible field has its
//! own validation path (mint Pubkey, price_provider enum), and that
//! errors surface as `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use yog_core::{
    RepositoryError,
    domain::{PriceProvider, TokenPrice},
};

use super::TokenPriceRow;

const VALID_MINT: &str = "So11111111111111111111111111111111111111112";

fn valid_row() -> TokenPriceRow {
    TokenPriceRow {
        mint: VALID_MINT.into(),
        price_usd: Decimal::new(12345, 2), // 123.45
        price_provider: "jupiter".into(),
        confidence: Some(0.95),
        fetched_at: Utc::now(),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_price_with_all_fields_mapped() {
    let row = valid_row();
    let fetched_at = row.fetched_at;

    let price = TokenPrice::try_from(row).expect("valid row should convert");

    assert_eq!(price.mint.to_string(), VALID_MINT);
    assert_eq!(price.price_usd, Decimal::new(12345, 2));
    assert_eq!(price.price_provider, PriceProvider::Jupiter);
    assert_eq!(price.confidence, Some(0.95));
    assert_eq!(price.fetched_at, fetched_at);
}

#[test]
fn try_from_with_none_confidence_returns_price_with_none() {
    let row = TokenPriceRow {
        confidence: None,
        ..valid_row()
    };

    let price = TokenPrice::try_from(row).expect("None confidence should convert");

    assert!(price.confidence.is_none());
}

#[test]
fn try_from_preserves_distinct_price_and_timestamp() {
    // Non-trivial Decimal value + distinct timestamp to verify the
    // passthrough doesn't drop or swap anything.
    let price_usd = Decimal::new(987654321, 5); // 9876.54321
    let fetched_at = Utc::now() + Duration::seconds(123);
    let row = TokenPriceRow {
        price_usd,
        fetched_at,
        ..valid_row()
    };

    let price = TokenPrice::try_from(row).expect("valid row should convert");

    assert_eq!(price.price_usd, price_usd);
    assert_eq!(price.fetched_at, fetched_at);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_mint_returns_integrity() {
    let row = TokenPriceRow {
        mint: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = TokenPrice::try_from(row).expect_err("invalid mint should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── PriceProvider enum mapping ─────────────────────────────────────────

#[test]
fn try_from_maps_all_known_price_providers() {
    for (raw, expected) in [
        ("jupiter", PriceProvider::Jupiter),
        ("helius", PriceProvider::Helius),
        ("fallback", PriceProvider::Fallback),
    ] {
        let row = TokenPriceRow {
            price_provider: raw.into(),
            ..valid_row()
        };
        let price = TokenPrice::try_from(row).expect("known source should convert");
        assert_eq!(
            price.price_provider, expected,
            "wire value {raw} should map to {expected:?}"
        );
    }
}

#[test]
fn try_from_invalid_price_provider_returns_integrity_with_value() {
    let row = TokenPriceRow {
        price_provider: "definitely_not_a_source".into(),
        ..valid_row()
    };
    let err = TokenPrice::try_from(row).expect_err("unknown price_provider should fail");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid price_provider") && msg.contains("definitely_not_a_source"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}
