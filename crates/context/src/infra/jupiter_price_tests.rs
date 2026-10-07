//! Unit tests for the Jupiter price client: the projection of an entry, its
//! deserialisation, the 429 retries, and the answer — against a hand-rolled
//! local HTTP server.

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use yog_bootstrap::SecretKey;

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

    // Distinct values per field: catches a field swap.
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
    // The field is absent: works only through `#[serde(default)]`.
    let body = r#"{}"#;
    let entry: JupiterPriceEntry = serde_json::from_str(body).expect("valid JSON");
    assert_eq!(entry.usd_price, None);
}

#[test]
fn entry_ignores_unknown_fields() {
    // Extra V3 fields are ignored: guards against `deny_unknown_fields`.
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
    // The HTTP-date form is not handled: our own backoff applies.
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
    // A ten-minute Retry-After is capped.
    let delay = rate_limit_backoff(0, Some(std::time::Duration::from_secs(600)));
    assert_eq!(delay, RATE_LIMIT_MAX_BACKOFF);
}

// ── 429 handling: retry loop against a local HTTP server ────────────

/// Serve `responses` on a fresh localhost listener, one connection each, and
/// return the base URL. A request beyond the script is refused: the listener
/// closes with it.
fn serve_scripted_responses(responses: Vec<String>) -> String {
    let untimed = responses
        .into_iter()
        .map(|response| (Duration::ZERO, response))
        .collect();
    serve_timed_responses(untimed).0
}

/// When each request of a timed server arrived, in order.
type Arrivals = Arc<Mutex<Vec<Instant>>>;

/// Same, each response written after its delay, and the arrival of each
/// request recorded once its head is read.
fn serve_timed_responses(responses: Vec<(Duration, String)>) -> (String, Arrivals) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind localhost");
    let base_url = format!("http://{}", listener.local_addr().expect("local addr"));
    let arrivals = Arrivals::default();
    let recorded = Arc::clone(&arrivals);

    std::thread::spawn(move || {
        for (delay, response) in responses {
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
            recorded.lock().unwrap().push(Instant::now());
            std::thread::sleep(delay);
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
    });

    (base_url, arrivals)
}

/// The time between two consecutive arrivals.
fn gaps(arrivals: &Arrivals) -> Vec<Duration> {
    let arrivals = arrivals.lock().unwrap();
    arrivals.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// A client at `per_minute`, which sets the spacing between two chunks.
fn client_at(base_url: String, per_minute: u32) -> JupiterPriceClient {
    JupiterPriceClient::new(
        base_url,
        SecretKey::for_tests("test-key"),
        NonZeroU32::new(per_minute).expect("non-zero"),
    )
}

/// A client spaced by about a millisecond, for the tests that are not about
/// the spacing.
fn unpaced_client(base_url: String) -> JupiterPriceClient {
    client_at(base_url, 60_000)
}

fn response_429(retry_after_secs: u64) -> String {
    format!(
        "HTTP/1.1 429 Too Many Requests\r\nretry-after: {retry_after_secs}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
    )
}

fn response_500() -> String {
    "HTTP/1.1 500 Internal Server Error\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
        .to_string()
}

/// A body pricing `mint` alone.
fn price_body(mint: Pubkey) -> String {
    format!(r#"{{ "{mint}": {{ "usdPrice": 1.0 }} }}"#)
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
    // 429 (Retry-After: 0 keeps it instant), then 200.
    let base_url = serve_scripted_responses(vec![response_429(0), response_200(&body)]);

    let client = unpaced_client(base_url);
    let answer = client
        .fetch_prices(&[mint], &CancellationToken::new())
        .await
        .expect("Ok expected");

    assert_eq!(answer.priced.len(), 1, "the retried chunk yields its price");
    assert_eq!(answer.priced[0].mint, mint);
    assert_eq!(answer.priced[0].price_usd, dec("1.5"));
    assert!(answer.unpriced.is_empty());
}

#[tokio::test]
async fn chunk_rate_limited_on_every_attempt_is_skipped() {
    let responses = (0..RATE_LIMIT_MAX_ATTEMPTS)
        .map(|_| response_429(0))
        .collect();
    let base_url = serve_scripted_responses(responses);

    let client = unpaced_client(base_url);
    let answer = client
        .fetch_prices(&[pk(21)], &CancellationToken::new())
        .await
        .expect("Ok expected");

    // Attempts exhausted: skipped, not an error, and not unpriced either.
    assert!(answer.priced.is_empty());
    assert!(
        answer.unpriced.is_empty(),
        "a chunk given up says nothing about its mints"
    );
}

#[tokio::test]
async fn every_mint_of_an_answered_chunk_is_priced_or_unpriced() {
    let (with_price, null_price, no_field, no_entry) = (pk(30), pk(31), pk(32), pk(33));
    // The three shapes of "no price": null, field absent, entry absent.
    let body = format!(
        r#"{{
          "{with_price}": {{ "usdPrice": 2.0 }},
          "{null_price}": {{ "usdPrice": null }},
          "{no_field}": {{ "liquidity": 0.42 }}
        }}"#
    );
    let base_url = serve_scripted_responses(vec![response_200(&body)]);

    let client = unpaced_client(base_url);
    let answer = client
        .fetch_prices(
            &[with_price, null_price, no_field, no_entry],
            &CancellationToken::new(),
        )
        .await
        .expect("Ok expected");

    assert_eq!(
        answer.priced.iter().map(|p| p.mint).collect::<Vec<_>>(),
        vec![with_price]
    );
    let mut unpriced = answer.unpriced;
    unpriced.sort();
    let mut expected = vec![null_price, no_field, no_entry];
    expected.sort();
    assert_eq!(unpriced, expected, "a missing entry is unpriced too");
}

#[tokio::test]
async fn an_answer_without_a_single_price_says_nothing() {
    // Four degraded 200s, none a verdict on the mints asked: an empty map,
    // all-null entries, an error body (it deserialises, keyed `error`), and a
    // price for a mint not asked.
    let (a, b) = (pk(40), pk(41));
    let all_null = format!(r#"{{ "{a}": {{ "usdPrice": null }}, "{b}": {{}} }}"#);
    let stray = format!(r#"{{ "{}": {{ "usdPrice": 1.0 }} }}"#, pk(42));
    let base_url = serve_scripted_responses(vec![
        response_200("{}"),
        response_200(&all_null),
        response_200(r#"{ "error": { "message": "upstream unavailable" } }"#),
        response_200(&stray),
    ]);

    let client = unpaced_client(base_url);
    for shape in [
        "empty map",
        "every entry without a price",
        "error body",
        "a price for a mint not asked",
    ] {
        let answer = client
            .fetch_prices(&[a, b], &CancellationToken::new())
            .await
            .expect("Ok expected");

        assert!(answer.priced.is_empty(), "{shape}");
        assert!(
            answer.unpriced.is_empty(),
            "{shape}: a degraded answer must not hold its mints back"
        );
    }
}

#[tokio::test]
async fn a_chunk_given_up_reports_nothing_beside_one_that_was_answered() {
    // A chunk of 50 answered with one price, then a chunk of one given up.
    let mints: Vec<Pubkey> = (1..=51).map(pk).collect();
    let (answered, given_up) = mints.split_at(JUPITER_BATCH_MAX);
    let body = format!(r#"{{ "{}": {{ "usdPrice": 3.0 }} }}"#, answered[0]);
    let responses = std::iter::once(response_200(&body))
        .chain((0..RATE_LIMIT_MAX_ATTEMPTS).map(|_| response_429(0)))
        .collect();
    let base_url = serve_scripted_responses(responses);

    let client = unpaced_client(base_url);
    let answer = client
        .fetch_prices(&mints, &CancellationToken::new())
        .await
        .expect("Ok expected");

    assert_eq!(answer.priced.len(), 1);
    assert_eq!(answer.priced[0].mint, answered[0]);
    assert_eq!(answer.unpriced.len(), JUPITER_BATCH_MAX - 1);
    assert!(
        !answer.unpriced.contains(&given_up[0]),
        "the mint of the chunk given up is not unpriced: nobody answered for it"
    );
}

// ── Spacing ─────────────────────────────────────────────────────────

#[test]
fn the_free_tier_is_one_request_every_1_1_s() {
    let free_tier = NonZeroU32::new(60).expect("non-zero");
    assert_eq!(spacing_under(free_tier), Duration::from_millis(1100));
}

/// Mutation this is written against: the `sleep_until` removed, and the three
/// chunks leave at once.
#[tokio::test]
async fn chunks_are_spaced_by_the_rate_limit() {
    // Three chunks: 50, 50 and 1 mints, each answered with one price.
    let mints: Vec<Pubkey> = (1..=101).map(pk).collect();
    let responses = mints
        .chunks(JUPITER_BATCH_MAX)
        .map(|chunk| response_200(&price_body(chunk[0])))
        .collect();
    let client = client_at(serve_scripted_responses(responses), 600);
    let spacing = client.request_spacing();
    assert_eq!(spacing, Duration::from_millis(110), "premise");

    let start = Instant::now();
    let answer = client
        .fetch_prices(&mints, &CancellationToken::new())
        .await
        .expect("Ok expected");
    let elapsed = start.elapsed();

    assert_eq!(answer.priced.len(), 3, "every chunk was asked");
    assert!(
        elapsed >= spacing * 2,
        "three chunks take two spacings at least, took {elapsed:?}"
    );
}

/// Mutation this is written against: `next_start` taken once the answer is
/// in, which adds the server's 400 ms to the gap.
#[tokio::test]
async fn the_spacing_runs_from_one_start_to_the_next() {
    // Two chunks, 1 s apart at 66 per minute, each answered in 400 ms.
    let mints: Vec<Pubkey> = (1..=51).map(pk).collect();
    let latency = Duration::from_millis(400);
    let responses = mints
        .chunks(JUPITER_BATCH_MAX)
        .map(|chunk| (latency, response_200(&price_body(chunk[0]))))
        .collect();
    let (base_url, arrivals) = serve_timed_responses(responses);
    let client = client_at(base_url, 66);
    assert_eq!(client.request_spacing(), Duration::from_secs(1), "premise");

    client
        .fetch_prices(&mints, &CancellationToken::new())
        .await
        .expect("Ok expected");

    let gaps = gaps(&arrivals);
    assert_eq!(gaps.len(), 1, "both chunks were asked");
    assert!(
        gaps[0] < Duration::from_millis(1300),
        "1 s from start to start, not 1.4 s from the answer: {gaps:?}"
    );
}

/// Mutation this is written against: the retry waiting its backoff alone,
/// which `Retry-After: 0` makes nothing.
#[tokio::test]
async fn a_retry_takes_its_slot_like_any_request() {
    // A first chunk refused then retried, and a second chunk: three requests,
    // 300 ms apart at 220 per minute.
    let mints: Vec<Pubkey> = (1..=51).map(pk).collect();
    let responses = vec![
        (Duration::ZERO, response_429(0)),
        (Duration::ZERO, response_200(&price_body(mints[0]))),
        (
            Duration::ZERO,
            response_200(&price_body(mints[JUPITER_BATCH_MAX])),
        ),
    ];
    let (base_url, arrivals) = serve_timed_responses(responses);
    let client = client_at(base_url, 220);
    let spacing = client.request_spacing();
    assert_eq!(spacing, Duration::from_millis(300), "premise");

    let answer = client
        .fetch_prices(&mints, &CancellationToken::new())
        .await
        .expect("Ok expected");

    assert_eq!(
        answer.priced.len(),
        2,
        "the retry and the second chunk answered"
    );
    let gaps = gaps(&arrivals);
    assert_eq!(gaps.len(), 2, "three requests");
    assert!(
        gaps.iter().all(|gap| *gap >= spacing * 4 / 5),
        "every request waits its slot, the retry included: {gaps:?}"
    );
}

/// Mutation this is written against: the `cancelled()` arm removed, and the
/// client waits out its minute before the second chunk.
#[tokio::test]
async fn a_stop_between_two_chunks_returns_what_was_answered() {
    // Two chunks, a minute apart; only the first is scripted.
    let mints: Vec<Pubkey> = (1..=51).map(pk).collect();
    let script = vec![(Duration::ZERO, response_200(&price_body(mints[0])))];
    let (base_url, arrivals) = serve_timed_responses(script);
    let client = client_at(base_url, 1);

    // Whether it lands during the first request or in the pause after it,
    // the pause that follows hears it.
    let shutdown = CancellationToken::new();
    let stop = shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        stop.cancel();
    });

    let answer = tokio::time::timeout(
        Duration::from_secs(5),
        client.fetch_prices(&mints, &shutdown),
    )
    .await
    .expect("the stop ends the fetch, not the minute's wait")
    .expect("Ok expected");

    assert_eq!(
        answer.priced.iter().map(|p| p.mint).collect::<Vec<_>>(),
        vec![mints[0]],
        "the first chunk's answer is kept"
    );
    assert_eq!(answer.unpriced.len(), JUPITER_BATCH_MAX - 1);
    assert!(
        !answer.unpriced.contains(&mints[JUPITER_BATCH_MAX]),
        "the chunk never sent says nothing about its mint"
    );
    assert_eq!(
        arrivals.lock().unwrap().len(),
        1,
        "the second chunk is never sent, not sent and refused"
    );
}

/// Mutation this is written against: the stop raced against the pause alone,
/// and the client waits for the request in flight — 15 s of `http_client`.
#[tokio::test]
async fn a_stop_during_a_request_drops_it_and_keeps_what_came_before() {
    // Two chunks back to back; Jupiter answers the second after 30 s.
    let mints: Vec<Pubkey> = (1..=51).map(pk).collect();
    let script = vec![
        (Duration::ZERO, response_200(&price_body(mints[0]))),
        (
            Duration::from_secs(30),
            response_200(&price_body(mints[JUPITER_BATCH_MAX])),
        ),
    ];
    let (base_url, arrivals) = serve_timed_responses(script);
    let client = unpaced_client(base_url);

    let shutdown = CancellationToken::new();
    let stop = shutdown.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        stop.cancel();
    });

    let answer = tokio::time::timeout(
        Duration::from_secs(5),
        client.fetch_prices(&mints, &shutdown),
    )
    .await
    .expect("the stop drops the request in flight")
    .expect("Ok expected");

    assert_eq!(
        arrivals.lock().unwrap().len(),
        2,
        "premise: the second was sent"
    );
    assert_eq!(
        answer.priced.iter().map(|p| p.mint).collect::<Vec<_>>(),
        vec![mints[0]],
        "the first chunk's answer is kept"
    );
    assert!(
        !answer.unpriced.contains(&mints[JUPITER_BATCH_MAX]),
        "the chunk dropped says nothing about its mint"
    );
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

// ── Redaction: the two error kinds a live server can produce ────────

/// A non-2xx status and an undecodable body, through a URL carrying a secret.
/// Here because the only local server is here; the connect failure is in
/// `infra/source_error_tests.rs`.
#[tokio::test]
async fn a_status_and_a_decode_failure_are_classified_without_leaking_the_secret() {
    const SECRET: &str = "SECRET-DE-TEST-a1b2c3d4";

    let base = serve_scripted_responses(vec![response_500(), response_200("pas du json")]);
    let url = format!("{base}/?api-key={SECRET}");

    // ⚠️ `http_client()`, not `Client::new()`: without its timeout, a mishap
    // in the scripted server hangs the test instead of failing it.

    // 1. Non-2xx → transport error, secret gone.
    let raw_status = crate::infra::http_client()
        .get(&url)
        .send()
        .await
        .expect("the scripted server answers")
        .error_for_status()
        .expect_err("500 must be an error");
    // Premise: if reqwest stopped attaching the URL, the check below would
    // prove nothing.
    assert!(raw_status.to_string().contains(SECRET), "{raw_status}");
    let status_err = SourceError::from(raw_status);
    assert!(matches!(status_err, SourceError::Http(_)), "{status_err}");
    assert!(!status_err.to_string().contains(SECRET), "{status_err}");

    // 2. 2xx with a body that is not JSON → decode error, secret gone.
    let raw_decode = crate::infra::http_client()
        .get(&url)
        .send()
        .await
        .expect("the scripted server answers")
        .json::<HashMap<String, JupiterPriceEntry>>()
        .await
        .expect_err("that body is not JSON");
    assert!(raw_decode.to_string().contains(SECRET), "{raw_decode}");
    let decode_err = SourceError::from(raw_decode);
    assert!(matches!(decode_err, SourceError::Decode(_)), "{decode_err}");
    assert!(!decode_err.to_string().contains(SECRET), "{decode_err}");
}
