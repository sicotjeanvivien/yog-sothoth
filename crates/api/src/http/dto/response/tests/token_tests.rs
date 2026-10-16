//! `EmbeddedTokenResponse::from_sources` — its fallback when metadata is absent,
//! and its camelCase wire shape — then `logo_uri`, which `TokenResponse::new`
//! applies too, and the tag of each price provider.

use chrono::{TimeZone, Utc};
use rust_decimal::Decimal;
use solana_pubkey::Pubkey;
use yog_core::domain::{self, MetadataProvider, PriceProvider, TokenMetadata, TokenPrice};

use super::price_provider_response::PriceProviderResponse;
use super::{EmbeddedTokenResponse, TokenResponse, logo_uri};
use crate::testing::wire_tag;

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
        metadata_provider: MetadataProvider::HeliusDas,
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
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(mint())), None);
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

    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(meta), None);
    let j = to_json(&dto);

    assert!(j["symbol"].is_null());
    assert!(j["name"].is_null());
    assert!(j["logoUri"].is_null());
    assert_eq!(j["decimals"], 6); // decimals still come from metadata
}

#[test]
fn empty_logo_uri_serializes_as_null() {
    // DAS sometimes reports an empty image string. The API contract is
    // "URL or null", so an empty logo must surface as null, not "".
    let mut meta = full_metadata(mint());
    meta.logo_uri = Some(String::new());

    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(meta), None);
    let j = to_json(&dto);

    assert!(j["logoUri"].is_null(), "empty logo must be null, not \"\"");
}

#[test]
fn embedded_token_filters_its_logo() {
    // Mutation: `from_sources` keeping any non-empty logo.
    let mut meta = full_metadata(mint());
    meta.logo_uri = Some("token-image.png".to_string());

    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(meta), None);

    assert!(to_json(&dto)["logoUri"].is_null());
}

// ── Branch 2: metadata absent ────────────────────────────────────────

#[test]
fn falls_back_when_metadata_is_none() {
    // Fresh pool whose mints have not been enriched by yog-context yet.
    // The dashboard must still see a usable payload.
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), None, None);
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
    let dto =
        EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(other_mint())), None);
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert_ne!(j["mint"], other_mint().to_string());
}

// ── Price independence ───────────────────────────────────────────────

#[test]
fn price_is_attached_when_present() {
    let dto = EmbeddedTokenResponse::from_sources(
        Some(mint()),
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
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(mint())), None);
    let j = to_json(&dto);

    assert!(j["price"].is_null());
}

#[test]
fn price_attaches_even_when_metadata_absent() {
    // Edge case: price exists before metadata. Could happen if the
    // price worker races ahead of the metadata worker. The response
    // must still attach the price.
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), None, Some(sample_price(mint())));
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
        Some(mint()),
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

// ── Logo ─────────────────────────────────────────────────────────────

#[test]
fn logo_uri_keeps_only_an_http_url() {
    // Mutations: back to the empty-string filter alone, no scheme check
    // (`ipfs://@`, which `z.url()` refuses), a `trim()` before parsing (U+0085),
    // and an `http(s)://` prefix check in place of the parse (port 65536).
    for refused in [
        "d",
        "token-image.png",
        "/_next/static/media/icon_solana_16px_bold_dark_s.62336f77.1255ba95.webp",
        "HaH5hiDBTxzXUcd7bRXbshrZ3BtcNQA8nhpd6H45iinu",
        "sgrs.debot.ai/logo/d09738ba72e94446ee1bc4d606bce723.png",
        "src=\"https://sgrs.debot.ai/logo/d09738ba72e94446ee1bc4d606bce723.png\"",
        "ipfs://@",
        "ipfs://bafybeihi7twfp4lljqegiyg6xlojwrrs7r6qrpdz24g7t63zn37qovumyu",
        "blob:https://x.com/5d74a49d-97ac-4c85-8f2c-27a1447c2c13",
        "\u{85}https://example.test/logo.png",
        "https://example.test:65536/logo.png",
    ] {
        assert_eq!(logo_uri(Some(refused.to_string())), None, "{refused:?}");
    }

    for kept in [
        "https://example.test/logo.png",
        "http://example.test/logo.png",
    ] {
        assert_eq!(logo_uri(Some(kept.to_string())).as_deref(), Some(kept));
    }
}

#[test]
fn token_response_filters_its_logo() {
    // Mutation: `TokenResponse::new` passing the raw logo through.
    let mut meta = full_metadata(mint());
    meta.logo_uri = Some("token-image.png".to_string());

    let j = serde_json::to_value(TokenResponse::new(meta, None)).expect("DTO must serialize");

    assert!(j["logoUri"].is_null());
}

#[test]
fn price_provider_matches_the_domain() {
    use domain::PriceProvider::*;
    for v in [Jupiter, Helius, Fallback] {
        assert_eq!(wire_tag(PriceProviderResponse::from(v)), v.as_str());
    }
}
