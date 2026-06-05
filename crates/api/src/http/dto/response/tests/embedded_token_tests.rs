//! Unit tests for `EmbeddedTokenResponse::from_sources`.
//!
//! Focused on the fallback branch — the only piece of conditional
//! logic in the response DTO layer. Also pins the camelCase wire
//! shape, since a misplaced `rename_all` would break frontend
//! consumers silently.

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::domain::{PriceProvider, TokenMetadata, TokenPrice};

use super::EmbeddedTokenResponse;

// ── Helpers ──────────────────────────────────────────────────────────

fn mint() -> Pubkey {
    Pubkey::new_from_array([7u8; 32])
}

fn other_mint() -> Pubkey {
    // Distinct from `mint()` — used to verify the function does not
    // read the mint off the metadata struct.
    Pubkey::new_from_array([42u8; 32])
}

fn full_metadata(m: Pubkey) -> TokenMetadata {
    TokenMetadata {
        mint: m,
        symbol: Some("USDC".to_string()),
        name: Some("USD Coin".to_string()),
        decimals: 6,
        logo_uri: Some("https://example.test/usdc.png".to_string()),
        metadata_source: "helius_das".to_string(),
        fetched_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        last_refresh_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
    }
}

fn sample_price(m: Pubkey) -> TokenPrice {
    TokenPrice {
        mint: m,
        price_usd: Decimal::new(100, 0),
        price_provider: PriceProvider::Helius,
        fetched_at: Utc.timestamp_opt(1_700_000_500, 0).unwrap(),
        confidence: Some(6.0),
    }
}

/// Deserialize the DTO to a generic `serde_json::Value` so the test
/// can inspect the wire field names without coupling to the private
/// struct shape.
fn to_json(dto: &EmbeddedTokenResponse) -> serde_json::Value {
    serde_json::to_value(dto).expect("DTO must serialize")
}

// ── Branch 1: metadata present ───────────────────────────────────────

#[test]
fn uses_metadata_fields_when_present() {
    let dto = EmbeddedTokenResponse::from_sources(mint(), Some(full_metadata(mint())), None);
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert_eq!(j["symbol"], "USDC");
    assert_eq!(j["name"], "USD Coin");
    assert_eq!(j["decimals"], 6);
    assert_eq!(j["logoUri"], "https://example.test/usdc.png");
    assert!(j["price"].is_null());
}

#[test]
fn metadata_with_null_symbol_and_name_is_preserved() {
    // DAS may return decimals only, with no Metaplex metadata. The
    // dashboard must see symbol/name as null, NOT default strings.
    let mut meta = full_metadata(mint());
    meta.symbol = None;
    meta.name = None;
    meta.logo_uri = None;

    let dto = EmbeddedTokenResponse::from_sources(mint(), Some(meta), None);
    let j = to_json(&dto);

    assert!(j["symbol"].is_null());
    assert!(j["name"].is_null());
    assert!(j["logoUri"].is_null());
    assert_eq!(j["decimals"], 6); // decimals still come from metadata
}

// ── Branch 2: metadata absent ────────────────────────────────────────

#[test]
fn falls_back_when_metadata_is_none() {
    // Fresh pool whose mints have not been enriched by yog-context yet.
    // The dashboard must still see a usable payload.
    let dto = EmbeddedTokenResponse::from_sources(mint(), None, None);
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert!(j["symbol"].is_null());
    assert!(j["name"].is_null());
    assert_eq!(
        j["decimals"], 0,
        "fallback decimals must be 0, not null — the frontend treats decimals as a number"
    );
    assert!(j["logoUri"].is_null());
    assert!(j["price"].is_null());
}

// ── Mint preservation invariant ──────────────────────────────────────

#[test]
fn mint_in_response_comes_from_parameter_not_metadata() {
    // Regression guard: the function takes the mint as a parameter
    // precisely because metadata may be absent. If a refactor ever
    // makes it read the mint off the metadata struct, the two paths
    // become inconsistent. This test pins the parameter as the
    // source of truth even when metadata is present.
    let dto = EmbeddedTokenResponse::from_sources(mint(), Some(full_metadata(other_mint())), None);
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert_ne!(j["mint"], other_mint().to_string());
}

// ── Price independence ───────────────────────────────────────────────

#[test]
fn price_is_attached_when_present() {
    let dto = EmbeddedTokenResponse::from_sources(
        mint(),
        Some(full_metadata(mint())),
        Some(sample_price(mint())),
    );
    let j = to_json(&dto);

    assert!(j["price"].is_object());
    assert!(j["price"]["usd"].is_string() || j["price"]["usd"].is_number());
    assert!(j["price"]["provider"].is_string());
    // `fetched_at` — camelCase guard on the nested DTO too.
    assert!(j["price"]["fetchedAt"].is_string());
}

#[test]
fn price_is_null_when_absent() {
    let dto = EmbeddedTokenResponse::from_sources(mint(), Some(full_metadata(mint())), None);
    let j = to_json(&dto);

    assert!(j["price"].is_null());
}

#[test]
fn price_attaches_even_when_metadata_absent() {
    // Edge case: price exists before metadata. Could happen if the
    // price worker races ahead of the metadata worker. The response
    // must still attach the price.
    let dto = EmbeddedTokenResponse::from_sources(mint(), None, Some(sample_price(mint())));
    let j = to_json(&dto);

    assert!(j["price"].is_object());
    assert!(j["symbol"].is_null());
    assert_eq!(j["decimals"], 0);
}

// ── Wire shape guard ─────────────────────────────────────────────────

#[test]
fn wire_field_names_are_camel_case() {
    // A misplaced `#[serde(rename_all = ...)]` would surface here.
    // The Next.js dashboard parses these payloads with zod and would
    // fail at runtime — this test catches it at `cargo test` time.
    let dto = EmbeddedTokenResponse::from_sources(
        mint(),
        Some(full_metadata(mint())),
        Some(sample_price(mint())),
    );
    let j = to_json(&dto);

    // Camel case fields that are NOT snake_case in JSON:
    assert!(j.get("logoUri").is_some(), "expected field `logoUri`");
    assert!(j.get("logo_uri").is_none(), "snake_case leaked");

    // Other fields with no case difference but still pinned:
    for k in ["mint", "symbol", "name", "decimals", "price"] {
        assert!(j.get(k).is_some(), "expected field `{k}`");
    }
}
