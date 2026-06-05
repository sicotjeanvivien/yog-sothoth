//! Unit tests for the Helius DAS client.
//!
//! Two surfaces are covered:
//!   - `into_fetched_metadata` — projection from the wire shape
//!     (`DasAsset`) to the worker view (`FetchedMetadata`), including
//!     every optional / fallback path.
//!   - `DasResponse` deserialization — locks down what we expect
//!     Helius to send, including the positional `null` entries and
//!     forward compatibility with unknown fields.
//!
//! The HTTP call itself (`fetch_asset_batch`) is not exercised here:
//! it is a thin reqwest pipeline whose only non-trivial step is the
//! decoding tested above.

use solana_pubkey::Pubkey;

use super::*;

/// Deterministic Pubkey for tests, matching the convention used
/// elsewhere in the workspace.
fn pk(seed: u8) -> Pubkey {
    Pubkey::new_from_array([seed; 32])
}

// ── into_fetched_metadata: projection logic ───────────────────────────

#[test]
fn happy_path_yields_all_fields_with_links_image_priority() {
    let mint = pk(1);
    let asset = DasAsset {
        id: mint.to_string(),
        content: Some(DasContent {
            metadata: Some(DasMetadata {
                name: Some("Alpha Token".to_string()),
                symbol: Some("ABC".to_string()),
            }),
            files: vec![DasFile {
                uri: Some("https://example.com/from-files.png".to_string()),
            }],
            links: Some(DasLinks {
                image: Some("https://example.com/from-links.png".to_string()),
            }),
        }),
        token_info: Some(DasTokenInfo { decimals: Some(9) }),
    };

    let result = into_fetched_metadata(asset).expect("expected Some");

    // Distinct values per field — catches any accidental field swap
    // in the projection.
    assert_eq!(result.mint, mint);
    assert_eq!(result.symbol.as_deref(), Some("ABC"));
    assert_eq!(result.name.as_deref(), Some("Alpha Token"));
    assert_eq!(result.decimals, 9);
    assert_eq!(
        result.logo_uri.as_deref(),
        Some("https://example.com/from-links.png"),
        "links.image must take precedence over files[0].uri",
    );
}

#[test]
fn drops_when_token_info_decimals_is_none() {
    let asset = DasAsset {
        id: pk(1).to_string(),
        content: Some(DasContent {
            metadata: None,
            files: vec![],
            links: None,
        }),
        token_info: Some(DasTokenInfo { decimals: None }),
    };

    assert!(into_fetched_metadata(asset).is_none());
}

#[test]
fn drops_when_token_info_is_absent() {
    let asset = DasAsset {
        id: pk(1).to_string(),
        content: Some(DasContent {
            metadata: None,
            files: vec![],
            links: None,
        }),
        token_info: None,
    };

    assert!(into_fetched_metadata(asset).is_none());
}

#[test]
fn drops_when_id_is_not_a_valid_pubkey() {
    let asset = DasAsset {
        id: "not-a-base58-pubkey!".to_string(),
        content: None,
        token_info: Some(DasTokenInfo { decimals: Some(6) }),
    };

    assert!(into_fetched_metadata(asset).is_none());
}

#[test]
fn handles_missing_content_with_nulls_on_optional_fields() {
    let mint = pk(2);
    let asset = DasAsset {
        id: mint.to_string(),
        content: None,
        token_info: Some(DasTokenInfo { decimals: Some(6) }),
    };

    let result = into_fetched_metadata(asset).expect("expected Some");

    assert_eq!(result.mint, mint);
    assert_eq!(result.symbol, None);
    assert_eq!(result.name, None);
    assert_eq!(result.decimals, 6);
    assert_eq!(result.logo_uri, None);
}

#[test]
fn handles_content_without_metadata_block() {
    let mint = pk(3);
    let asset = DasAsset {
        id: mint.to_string(),
        content: Some(DasContent {
            metadata: None,
            files: vec![],
            links: Some(DasLinks {
                image: Some("https://example.com/logo.png".to_string()),
            }),
        }),
        token_info: Some(DasTokenInfo { decimals: Some(8) }),
    };

    let result = into_fetched_metadata(asset).expect("expected Some");

    assert_eq!(result.symbol, None);
    assert_eq!(result.name, None);
    assert_eq!(result.decimals, 8);
    assert_eq!(
        result.logo_uri.as_deref(),
        Some("https://example.com/logo.png"),
    );
}

#[test]
fn falls_back_to_files_when_links_image_absent() {
    let mint = pk(4);
    let asset = DasAsset {
        id: mint.to_string(),
        content: Some(DasContent {
            metadata: None,
            files: vec![DasFile {
                uri: Some("ipfs://QmHash".to_string()),
            }],
            links: None,
        }),
        token_info: Some(DasTokenInfo { decimals: Some(6) }),
    };

    let result = into_fetched_metadata(asset).expect("expected Some");

    assert_eq!(result.logo_uri.as_deref(), Some("ipfs://QmHash"));
}

#[test]
fn skips_files_with_null_uri_to_find_first_usable_one() {
    let mint = pk(5);
    let asset = DasAsset {
        id: mint.to_string(),
        content: Some(DasContent {
            metadata: None,
            files: vec![
                DasFile { uri: None },
                DasFile {
                    uri: Some("https://example.com/second.png".to_string()),
                },
            ],
            links: None,
        }),
        token_info: Some(DasTokenInfo { decimals: Some(6) }),
    };

    let result = into_fetched_metadata(asset).expect("expected Some");

    assert_eq!(
        result.logo_uri.as_deref(),
        Some("https://example.com/second.png"),
    );
}

#[test]
fn returns_none_logo_when_no_source_yields_a_uri() {
    let mint = pk(6);
    let asset = DasAsset {
        id: mint.to_string(),
        content: Some(DasContent {
            metadata: None,
            files: vec![DasFile { uri: None }],
            links: Some(DasLinks { image: None }),
        }),
        token_info: Some(DasTokenInfo { decimals: Some(6) }),
    };

    let result = into_fetched_metadata(asset).expect("expected Some");

    assert_eq!(result.logo_uri, None);
}

// ── DasResponse: deserialization ─────────────────────────────────────

#[test]
fn das_response_deserializes_mixed_results_with_nulls() {
    let mint_a = pk(10);
    let mint_b = pk(11);
    let body = format!(
        r#"{{
          "jsonrpc": "2.0",
          "id": "yog-context",
          "result": [
            {{
              "id": "{mint_a}",
              "content": {{
                "metadata": {{ "name": "A Token", "symbol": "AAA" }},
                "files": [{{ "uri": "https://example.com/a.png" }}],
                "links": {{ "image": "https://example.com/a-link.png" }}
              }},
              "token_info": {{ "decimals": 6 }}
            }},
            null,
            {{
              "id": "{mint_b}",
              "content": null,
              "token_info": {{ "decimals": 9 }}
            }}
          ]
        }}"#
    );

    let response: DasResponse = serde_json::from_str(&body).expect("valid JSON");
    assert_eq!(
        response.result.len(),
        3,
        "expected three positional entries"
    );
    assert!(response.result[1].is_none(), "middle entry must be null");

    // Apply the full projection chain to assert end-to-end behaviour
    // of the deserialization + projection pipeline that lives inside
    // `fetch_asset_batch`.
    let projected: Vec<FetchedMetadata> = response
        .result
        .into_iter()
        .flatten()
        .filter_map(into_fetched_metadata)
        .collect();

    assert_eq!(projected.len(), 2);
    assert_eq!(projected[0].mint, mint_a);
    assert_eq!(projected[0].symbol.as_deref(), Some("AAA"));
    assert_eq!(projected[1].mint, mint_b);
    assert_eq!(projected[1].symbol, None);
    assert_eq!(projected[1].decimals, 9);
}

#[test]
fn das_response_deserializes_empty_result() {
    let body = r#"{ "jsonrpc": "2.0", "id": "yog-context", "result": [] }"#;
    let response: DasResponse = serde_json::from_str(body).expect("valid JSON");
    assert!(response.result.is_empty());
}

#[test]
fn das_response_ignores_unknown_fields() {
    let mint = pk(20);
    let body = format!(
        r#"{{
          "jsonrpc": "2.0",
          "id": "yog-context",
          "extraTopLevel": 42,
          "result": [
            {{
              "id": "{mint}",
              "interface": "FungibleToken",
              "ownership": {{ "frozen": false, "owner": "abc" }},
              "supply": {{ "print_max_supply": 0 }},
              "content": {{
                "metadata": {{ "name": "Z", "symbol": "Z", "description": "..." }},
                "files": [],
                "links": null
              }},
              "token_info": {{
                "decimals": 6,
                "symbol": "Z",
                "token_program": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
              }}
            }}
          ]
        }}"#
    );

    let response: DasResponse =
        serde_json::from_str(&body).expect("unknown fields must be silently ignored by serde");
    assert_eq!(response.result.len(), 1);
}
