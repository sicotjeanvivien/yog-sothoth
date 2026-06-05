//! Unit tests for `TryFrom<TokenMetadataRow> for TokenMetadata`.
//!
//! Pure parser tests, no DB. Build rows by hand, assert that valid
//! ones produce the right metadata, that each fallible field has
//! its own validation path (mint Pubkey, decimals i16 → u8), and
//! that errors surface as `RepositoryError::Integrity`.

use chrono::{Duration, Utc};
use yog_core::{
    RepositoryError,
    domain::{MetadataProvider, TokenMetadata},
};

use super::TokenMetadataRow;

const VALID_MINT: &str = "So11111111111111111111111111111111111111112";

fn valid_row() -> TokenMetadataRow {
    let now = Utc::now();
    TokenMetadataRow {
        mint: VALID_MINT.into(),
        symbol: Some("SOL".into()),
        name: Some("Wrapped SOL".into()),
        decimals: 9,
        logo_uri: Some("https://example.com/sol.png".into()),
        metadata_provider: "helius_das".into(),
        fetched_at: now,
        last_refresh_at: now + Duration::seconds(30),
    }
}

// ── Happy path ───────────────────────────────────────────────────────

#[test]
fn try_from_valid_row_returns_metadata_with_all_fields_mapped() {
    let row = valid_row();
    let fetched_at = row.fetched_at;
    let last_refresh_at = row.last_refresh_at;

    let metadata = TokenMetadata::try_from(row).expect("valid row should convert");

    assert_eq!(metadata.mint.to_string(), VALID_MINT);
    assert_eq!(metadata.symbol.as_deref(), Some("SOL"));
    assert_eq!(metadata.name.as_deref(), Some("Wrapped SOL"));
    assert_eq!(metadata.decimals, 9);
    assert_eq!(
        metadata.logo_uri.as_deref(),
        Some("https://example.com/sol.png")
    );
    assert_eq!(metadata.metadata_provider, MetadataProvider::HeliusDas);
    assert_eq!(metadata.fetched_at, fetched_at);
    assert_eq!(metadata.last_refresh_at, last_refresh_at);
}

#[test]
fn try_from_with_none_optionals_returns_metadata_with_none() {
    // The three Option<String> fields go through direct passthrough;
    // pin that None inputs produce None outputs (not, say, Some("")
    // from a misplaced unwrap_or_default).
    let row = TokenMetadataRow {
        symbol: None,
        name: None,
        logo_uri: None,
        ..valid_row()
    };

    let metadata = TokenMetadata::try_from(row).expect("None optionals should convert");

    assert!(metadata.symbol.is_none());
    assert!(metadata.name.is_none());
    assert!(metadata.logo_uri.is_none());
}

#[test]
fn try_from_preserves_distinct_timestamps_in_correct_fields() {
    // Distinct fetched_at and last_refresh_at — a field swap in
    // the TryFrom would surface here.
    let fetched_at = Utc::now();
    let last_refresh_at = fetched_at + Duration::seconds(123);
    let row = TokenMetadataRow {
        fetched_at,
        last_refresh_at,
        ..valid_row()
    };

    let metadata = TokenMetadata::try_from(row).expect("valid row should convert");

    assert_eq!(metadata.fetched_at, fetched_at);
    assert_eq!(metadata.last_refresh_at, last_refresh_at);
}

// ── Pubkey validation ────────────────────────────────────────────────

#[test]
fn try_from_invalid_mint_returns_integrity() {
    let row = TokenMetadataRow {
        mint: "not-a-pubkey".into(),
        ..valid_row()
    };
    let err = TokenMetadata::try_from(row).expect_err("invalid mint should fail");
    assert!(
        matches!(err, RepositoryError::Integrity(_)),
        "expected Integrity, got {err:?}"
    );
}

// ── decimals bounds (i16 → u8) ───────────────────────────────────────

#[test]
fn try_from_negative_decimals_returns_integrity_with_value() {
    let row = TokenMetadataRow {
        decimals: -1,
        ..valid_row()
    };
    let err =
        TokenMetadata::try_from(row).expect_err("negative decimals should fail u8 conversion");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid decimals") && msg.contains("-1"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}

#[test]
fn try_from_decimals_above_u8_max_returns_integrity_with_value() {
    let row = TokenMetadataRow {
        decimals: 300,
        ..valid_row()
    };
    let err =
        TokenMetadata::try_from(row).expect_err("decimals above 255 should fail u8 conversion");
    let msg = match err {
        RepositoryError::Integrity(m) => m,
        other => panic!("expected Integrity, got {other:?}"),
    };
    assert!(
        msg.contains("invalid decimals") && msg.contains("300"),
        "expected message to mention the field and the bad value, got: {msg}"
    );
}
