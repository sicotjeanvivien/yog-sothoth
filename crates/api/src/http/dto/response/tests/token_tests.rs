//! `EmbeddedTokenResponse::from_sources` — its fallback when metadata is absent,
//! and its camelCase wire shape — then `logo_uri`, which `TokenResponse::new`
//! applies too, and the tag of each price provider.

use chrono::{TimeZone, Utc};
use solana_pubkey::Pubkey;
use yog_core::domain::{self, MetadataProvider, TokenMetadata};

use super::price_provider_response::PriceProviderResponse;
use super::{EmbeddedTokenResponse, TokenResponse, logo_uri};
use crate::testing::wire_tag;

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

fn to_json(dto: &EmbeddedTokenResponse) -> serde_json::Value {
    serde_json::to_value(dto).expect("DTO must serialize")
}

#[test]
fn uses_metadata_fields_when_present() {
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(mint())));
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert_eq!(j["symbol"], "USDC");
    assert_eq!(j["name"], "USD Coin");
    assert_eq!(j["decimals"], 6);
    assert_eq!(j["logoUri"], "https://example.test/usdc.png");
}

#[test]
fn metadata_with_null_symbol_and_name_is_preserved() {
    // Decimals only, no Metaplex metadata: symbol and name null, not default strings.
    let mut meta = full_metadata(mint());
    meta.symbol = None;
    meta.name = None;
    meta.logo_uri = None;

    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(meta));
    let j = to_json(&dto);

    assert!(j["symbol"].is_null());
    assert!(j["name"].is_null());
    assert!(j["logoUri"].is_null());
    assert_eq!(j["decimals"], 6); // decimals still come from metadata
}

#[test]
fn empty_logo_uri_serializes_as_null() {
    // An empty image string surfaces as null: the contract is "URL or null".
    let mut meta = full_metadata(mint());
    meta.logo_uri = Some(String::new());

    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(meta));
    let j = to_json(&dto);

    assert!(j["logoUri"].is_null(), "empty logo must be null, not \"\"");
}

#[test]
fn embedded_token_filters_its_logo() {
    // Mutation: `from_sources` keeping any non-empty logo.
    let mut meta = full_metadata(mint());
    meta.logo_uri = Some("token-image.png".to_string());

    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(meta));

    assert!(to_json(&dto)["logoUri"].is_null());
}

#[test]
fn falls_back_when_metadata_is_none() {
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), None);
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert!(j["symbol"].is_null());
    assert!(j["name"].is_null());
    assert_eq!(
        j["decimals"], 0,
        "fallback decimals must be 0, not null — the frontend treats decimals as a number"
    );
    assert!(j["logoUri"].is_null());
}

#[test]
fn mint_in_response_comes_from_parameter_not_metadata() {
    // Mutation: the mint read off the metadata instead of the parameter.
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(other_mint())));
    let j = to_json(&dto);

    assert_eq!(j["mint"], mint().to_string());
    assert_ne!(j["mint"], other_mint().to_string());
}

#[test]
fn the_embedded_token_carries_no_price() {
    // Mutation: the `price` field back on the embedded view.
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(mint())));

    assert!(to_json(&dto).get("price").is_none());
}

#[test]
fn wire_field_names_are_camel_case() {
    // Mutation: a misplaced `#[serde(rename_all = ...)]`, which the web's zod refuses at runtime.
    let dto = EmbeddedTokenResponse::from_sources(Some(mint()), Some(full_metadata(mint())));
    let j = to_json(&dto);

    assert!(j.get("logoUri").is_some(), "expected field `logoUri`");
    assert!(j.get("logo_uri").is_none(), "snake_case leaked");

    for k in ["mint", "symbol", "name", "decimals"] {
        assert!(j.get(k).is_some(), "expected field `{k}`");
    }
}

#[test]
fn logo_uri_keeps_only_an_http_url() {
    // Mutations: the empty-string filter alone, no scheme check (`ipfs://@`), a `trim()`
    // before parsing (U+0085), a prefix check in place of the parse (port 65536).
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

/// Mutation: the price or its instant dropped, or the source misnamed.
#[test]
fn a_price_says_its_value_source_and_instant() {
    let price = super::price_response::TokenPriceResponse::from(domain::TokenPrice {
        mint: Pubkey::new_from_array([1; 32]),
        price_usd: rust_decimal::Decimal::from(2),
        price_provider: domain::PriceProvider::Jupiter,
        confidence: None,
        fetched_at: Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap(),
    });

    assert_eq!(
        serde_json::to_value(price).unwrap(),
        serde_json::json!({
            "usd": "2",
            "provider": "jupiter",
            "fetchedAt": "2026-10-25T12:00:00Z",
        })
    );
}
