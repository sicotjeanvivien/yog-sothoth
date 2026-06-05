//! Unit tests for the Jupiter price client.
//!
//! Two surfaces are covered:
//!   - `into_fetched_price` — projection from `(mint_str, JupiterPriceEntry)`
//!     to the worker view (`FetchedPrice`), with its two drop conditions
//!     (no usable price, unparseable mint).
//!   - `JupiterPriceEntry` deserialization — locks down the three real-world
//!     "no price" cases (value present, `null`, field absent), plus
//!     forward compatibility with the extra fields V3 returns
//!     (`createdAt`, `liquidity`, `blockId`, `decimals`, …).
//!
//! The HTTP call itself (`fetch_prices_batch`) is not exercised: it is
//! a thin reqwest pipeline whose non-trivial parts are tested above.

use std::collections::HashMap;
use std::str::FromStr;

use rust_decimal::Decimal;
use solana_pubkey::Pubkey;

use super::*;

fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal")
}

// ── into_fetched_price: projection ──────────────────────────────────

#[test]
fn happy_path_yields_mint_and_price() {
    let mint = pk(1);
    let entry = JupiterPriceEntry {
        usd_price: Some(dec("1.5")),
    };

    let result = into_fetched_price((mint.to_string(), entry)).expect("expected Some");

    // Distinct values per field — catches accidental field swap.
    assert_eq!(result.mint, mint);
    assert_eq!(result.price_usd, dec("1.5"));
}

#[test]
fn drops_when_usd_price_is_none() {
    let entry = JupiterPriceEntry { usd_price: None };

    assert!(into_fetched_price((pk(1).to_string(), entry)).is_none());
}

#[test]
fn drops_when_mint_string_is_not_a_valid_pubkey() {
    let entry = JupiterPriceEntry {
        usd_price: Some(dec("1.0")),
    };

    assert!(into_fetched_price(("not-a-base58-pubkey!".to_string(), entry)).is_none());
}

#[test]
fn preserves_high_precision_decimal() {
    // Memecoin-style price: very small value, many fractional digits.
    let mint = pk(2);
    let raw = "0.000000123456789012";
    let entry = JupiterPriceEntry {
        usd_price: Some(dec(raw)),
    };

    let result = into_fetched_price((mint.to_string(), entry)).expect("expected Some");

    assert_eq!(result.price_usd, dec(raw));
}

// ── JupiterPriceEntry: deserialization ──────────────────────────────

#[test]
fn entry_deserializes_present_price() {
    let body = r#"{ "usdPrice": 1.5 }"#;
    let entry: JupiterPriceEntry = serde_json::from_str(body).expect("valid JSON");
    assert_eq!(entry.usd_price, Some(dec("1.5")));
}

#[test]
fn entry_deserializes_null_price() {
    let body = r#"{ "usdPrice": null }"#;
    let entry: JupiterPriceEntry = serde_json::from_str(body).expect("valid JSON");
    assert_eq!(entry.usd_price, None);
}

#[test]
fn entry_deserializes_missing_price_field() {
    // The field is entirely ABSENT — only works because of
    // `#[serde(default)]` on the field. If someone removes that
    // attribute, this test breaks immediately.
    let body = r#"{}"#;
    let entry: JupiterPriceEntry = serde_json::from_str(body).expect("valid JSON");
    assert_eq!(entry.usd_price, None);
}

#[test]
fn entry_ignores_unknown_fields() {
    // Real V3 entries carry many extra fields we don't read.
    // serde must keep ignoring them silently — guard against
    // someone adding `#[serde(deny_unknown_fields)]` later.
    let body = r#"{
      "usdPrice": 1.0,
      "blockId": 42,
      "decimals": 6,
      "priceChange24h": -3.21,
      "liquidity": 123456.789,
      "createdAt": "2025-01-01T00:00:00Z",
      "launchpad": null
    }"#;

    let entry: JupiterPriceEntry = serde_json::from_str(body).expect("valid JSON");
    assert_eq!(entry.usd_price, Some(dec("1.0")));
}

// ── Full response: HashMap deserialization + projection ─────────────

#[test]
fn full_response_filters_to_priced_mints_only() {
    let mint_a = pk(10);
    let mint_b = pk(11);
    let mint_c = pk(12);

    let body = format!(
        r#"{{
          "{mint_a}": {{ "usdPrice": 0.999 }},
          "{mint_b}": {{ "usdPrice": null }},
          "{mint_c}": {{}}
        }}"#
    );

    let response: HashMap<String, JupiterPriceEntry> =
        serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(response.len(), 3, "all three entries deserialize");

    // Apply the projection pipeline the same way `fetch_prices_batch`
    // does, to assert end-to-end behaviour.
    let projected: Vec<FetchedPrice> = response
        .into_iter()
        .filter_map(into_fetched_price)
        .collect();

    assert_eq!(projected.len(), 1, "only the priced mint survives");
    assert_eq!(projected[0].mint, mint_a);
    assert_eq!(projected[0].price_usd, dec("0.999"));
}

#[test]
fn full_response_handles_empty_object() {
    let body = r#"{}"#;
    let response: HashMap<String, JupiterPriceEntry> =
        serde_json::from_str(body).expect("valid JSON");
    assert!(response.is_empty());

    let projected: Vec<FetchedPrice> = response
        .into_iter()
        .filter_map(into_fetched_price)
        .collect();
    assert!(projected.is_empty());
}
