//! What can be tested in a file whose header says it cannot be.
//!
//! The connection, the retry budget and `from_slot` need a server, and that is
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md`. `channel_endpoint` is the
//! exception: it is a pure function of the configured endpoint, it runs
//! **before** the loop, and what it refuses is the misconfiguration this path
//! makes possible.

use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> GrpcListener {
    GrpcListener::new(Endpoint::for_tests(url, None), 1, IngestScope::Pools)
}

#[test]
fn an_http_endpoint_is_accepted() {
    for url in [
        "https://grpc.example.com:443",
        "http://127.0.0.1:10000",
        "HTTPS://x.io",
    ] {
        assert!(
            listener(url).channel_endpoint().is_ok(),
            "{url} is a gRPC endpoint"
        );
    }
}

/// ⚠️ The defect this guard was written for, and it was found by *reading a
/// successful-looking run*: the gRPC path was launched against the `wss://` URL
/// of `.env`, produced ten retries with backoff, and that was taken for "a
/// transport error, as expected". `from_shared` accepts `wss://` — it is a
/// valid URI — so without this check the failure lands inside the retry loop,
/// two minutes away from its cause, and `run`'s doc-comment promises the
/// opposite.
#[test]
fn a_websocket_endpoint_is_refused_before_the_loop() {
    let detail = listener("wss://api.example.com")
        .channel_endpoint()
        .expect_err("a WebSocket URL is not a gRPC endpoint")
        .to_string();

    assert!(detail.contains("wss"), "names the scheme it saw: {detail}");
    assert!(detail.contains("https://"), "names what to write: {detail}");
    assert!(
        detail.contains("INGEST_SOURCE=rpc"),
        "names the source that does read it: {detail}"
    );
}

#[test]
fn a_url_without_a_scheme_is_refused_by_its_own_reason() {
    let detail = listener("grpc.example.com:443")
        .channel_endpoint()
        .expect_err("a schemeless URL is not a gRPC endpoint")
        .to_string();

    assert!(detail.contains("no scheme"), "{detail}");
}
