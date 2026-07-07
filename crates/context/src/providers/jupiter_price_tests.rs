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
//!   - 429 handling — `parse_retry_after` (header extraction) and
//!     `rate_limit_backoff` (delay policy), plus the retry loop
//!     end-to-end against a hand-rolled local HTTP server (no mock
//!     dependency): 429-then-200 recovers the chunk, all-429 gives it
//!     up as skip-and-log.
//!
//! The happy-path HTTP call itself is otherwise not exercised: it is
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

// ── 429 handling: Retry-After parsing + backoff policy ──────────────

fn headers_with_retry_after(value: &str) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::RETRY_AFTER,
        value.parse().expect("valid header value"),
    );
    headers
}

#[test]
fn retry_after_parses_delta_seconds() {
    let headers = headers_with_retry_after("2");
    assert_eq!(
        parse_retry_after(&headers),
        Some(std::time::Duration::from_secs(2))
    );
}

#[test]
fn retry_after_absent_yields_none() {
    let headers = reqwest::header::HeaderMap::new();
    assert_eq!(parse_retry_after(&headers), None);
}

#[test]
fn retry_after_http_date_form_yields_none() {
    // The HTTP-date form is valid per RFC 9110 but not handled — it
    // must fall back to our own backoff, not panic or mis-parse.
    let headers = headers_with_retry_after("Wed, 21 Oct 2026 07:28:00 GMT");
    assert_eq!(parse_retry_after(&headers), None);
}

#[test]
fn backoff_uses_server_retry_after_when_present() {
    let delay = rate_limit_backoff(0, Some(std::time::Duration::from_secs(3)));
    assert_eq!(delay, std::time::Duration::from_secs(3));
}

#[test]
fn backoff_grows_exponentially_without_retry_after() {
    assert_eq!(rate_limit_backoff(0, None), RATE_LIMIT_BASE_BACKOFF);
    assert_eq!(rate_limit_backoff(1, None), RATE_LIMIT_BASE_BACKOFF * 2);
}

#[test]
fn backoff_caps_a_hostile_retry_after() {
    // A server-provided Retry-After of ten minutes must not stall the
    // worker: the cap wins.
    let delay = rate_limit_backoff(0, Some(std::time::Duration::from_secs(600)));
    assert_eq!(delay, RATE_LIMIT_MAX_BACKOFF);
}

// ── 429 handling: retry loop against a local HTTP server ────────────

/// Serve `responses` on a fresh localhost listener, one connection per
/// response (each response closes its connection), and return the base
/// URL. Requests beyond the scripted responses are not served — a
/// client retrying more than expected fails loudly on connect/read.
fn serve_scripted_responses(responses: Vec<String>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind localhost");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));

    std::thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().expect("accept");
            // Drain the request head before answering.
            use std::io::{Read, Write};
            let mut buf = [0u8; 4096];
            let mut head = Vec::new();
            loop {
                let n = stream.read(&mut buf).expect("read request");
                head.extend_from_slice(&buf[..n]);
                if n == 0 || head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    base_url
}

fn response_429(retry_after_secs: u64) -> String {
    format!(
        "HTTP/1.1 429 Too Many Requests\r\nretry-after: {retry_after_secs}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
}

fn response_200(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn rate_limited_chunk_recovers_on_retry() {
    let mint = pk(20);
    let body = format!(r#"{{ "{mint}": {{ "usdPrice": 1.5 }} }}"#);
    // First call 429 (Retry-After: 0 keeps the test instant), second OK.
    let base_url = serve_scripted_responses(vec![response_429(0), response_200(&body)]);

    let client = JupiterPriceClient::new(base_url, "test-key".to_string());
    let fetched = client.fetch_prices(&[mint]).await.expect("Ok expected");

    assert_eq!(fetched.len(), 1, "the retried chunk yields its price");
    assert_eq!(fetched[0].mint, mint);
    assert_eq!(fetched[0].price_usd, dec("1.5"));
}

#[tokio::test]
async fn chunk_rate_limited_on_every_attempt_is_skipped() {
    let responses = (0..RATE_LIMIT_MAX_ATTEMPTS)
        .map(|_| response_429(0))
        .collect();
    let base_url = serve_scripted_responses(responses);

    let client = JupiterPriceClient::new(base_url, "test-key".to_string());
    let fetched = client.fetch_prices(&[pk(21)]).await.expect("Ok expected");

    // Attempts exhausted → skip-and-log, never a hard error.
    assert!(fetched.is_empty());
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
