use super::*;
use crate::providers::http_client;

/// Stands in for the API key. Deliberately unlike anything else in the
/// message, so `contains` cannot pass by accident.
///
/// A twin of this constant lives in `providers/jupiter_price_tests.rs`, which
/// covers the two error kinds this file cannot produce without a server. They
/// are independent on purpose: neither test should break because the other
/// changed its fixture.
const SECRET: &str = "SECRET-DE-TEST-a1b2c3d4";

/// The assertion this module exists for.
///
/// Port 1 on loopback is reserved and nothing listens there, so the request
/// fails to connect without a single packet leaving the machine — no server
/// to script, no network, nothing to flake. The error reqwest hands back
/// carries the URL, which is precisely the shape that wrote the Helius key
/// into 38 log lines on 2 September 2026.
///
/// Uses the crate's own `http_client()` rather than `Client::new()`: it is
/// the client the daemon actually ships, and its 5 s connect timeout bounds
/// this test on a host that *drops* traffic to port 1 instead of refusing it.
#[tokio::test]
async fn a_transport_failure_carries_neither_the_url_nor_its_secret() {
    let url = format!("http://127.0.0.1:1/?api-key={SECRET}");

    let raw = http_client()
        .get(&url)
        .send()
        .await
        .expect_err("port 1 must refuse the connection");

    // The premise: without the conversion, the secret *is* in there. If this
    // ever stops holding, the assertions below would pass while proving
    // nothing.
    assert!(
        raw.to_string().contains(SECRET),
        "reqwest no longer puts the URL in its error — this test's premise is \
         gone, and the conversion may no longer be what protects us: {raw}"
    );

    let converted = SourceError::from(raw);
    let rendered = converted.to_string();

    assert!(!rendered.contains(SECRET), "secret leaked: {rendered}");
    // The URL is gone in the shape that matters — the query string that
    // carries the key, and the URL form itself. Deliberately *not* asserting
    // the absence of `127.0.0.1`: the resolved address is not the secret this
    // invariant protects, and a purely diagnostic change in hyper-util (which
    // today writes its connect message without the peer address) would turn
    // that assertion red without any regression here.
    assert!(
        !rendered.contains("api-key="),
        "query string leaked: {rendered}"
    );
    assert!(!rendered.contains("http://"), "url leaked: {rendered}");
    assert!(matches!(converted, SourceError::Http(_)), "{rendered}");

    // And the message must still say *what* failed. reqwest renders only the
    // kind and the URL, never the cause, so stripping the URL leaves the
    // constant "error sending request" — identical for a refused connection,
    // a DNS failure, a TLS handshake and both timeouts. The conversion
    // appends the cause chain to compensate; today that reads
    //
    //   source HTTP error: error sending request: client error (Connect):
    //   tcp connect error: Connection refused (os error 111)
    //
    // Counting separators rather than matching that wording: what is asserted
    // is that *a cause was appended*, which is the thing that can regress.
    // The exact phrasing belongs to hyper and is free to change.
    assert!(
        rendered.matches(": ").count() >= 2,
        "the cause chain was not appended, so the message no longer says what \
         failed: {rendered}"
    );
}
