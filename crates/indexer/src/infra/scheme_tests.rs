//! What each path accepts, and what each refusal says.
//!
//! One test per transport and per reason: a test that only proved "refused"
//! would stay green while the message named the wrong source. Whether each
//! listener actually *calls* the check is a separate statement, tested beside
//! it. Case and the RFC 3986 shape of a scheme are [`SecretUrl::scheme`]'s, and
//! tested in `yog-bootstrap`.

use super::*;

fn check_url(raw: &str, transport: &Transport) -> Result<(), String> {
    check(&SecretUrl::for_tests(raw), transport)
}

// ── the WebSocket path ──────────────────────────────────────────────

#[test]
fn the_websocket_path_accepts_ws_and_wss() {
    for url in ["wss://api.example.com", "ws://127.0.0.1:8900", "WSS://x.io"] {
        assert_eq!(check_url(url, &WEBSOCKET), Ok(()), "{url}");
    }
}

/// The operator who tried Yellowstone and came back keeps an `https://` URL.
#[test]
fn the_websocket_path_refuses_a_grpc_url_and_says_which_source_reads_it() {
    let detail = check_url("https://grpc.example.com:443", &WEBSOCKET)
        .expect_err("an https endpoint is not a WebSocket");

    assert!(
        detail.contains("`https` scheme"),
        "names the scheme it saw: {detail}"
    );
    assert!(detail.contains("wss://"), "names what to write: {detail}");
    assert!(
        detail.contains("INGEST_SOURCE=grpc"),
        "names the source that does read it: {detail}"
    );
}

#[test]
fn the_websocket_path_refuses_a_url_without_a_scheme_by_its_own_reason() {
    let detail = check_url("api.example.com:443", &WEBSOCKET)
        .expect_err("a schemeless URL is not a WebSocket");

    assert!(detail.contains("no scheme"), "{detail}");
    assert!(
        !detail.contains("INGEST_SOURCE"),
        "no other source reads a URL with no scheme: {detail}"
    );
}

// ── the gRPC path ───────────────────────────────────────────────────

#[test]
fn the_grpc_path_accepts_http_and_https() {
    for url in [
        "https://grpc.example.com:443",
        "http://127.0.0.1:10000",
        "HTTPS://x.io",
    ] {
        assert_eq!(check_url(url, &GRPC), Ok(()), "{url}");
    }
}

/// `.env.example` ships a `wss://` `INGEST_STREAM_URL`; flipping only
/// `INGEST_SOURCE=grpc` keeps it.
#[test]
fn the_grpc_path_refuses_a_websocket_url_and_says_which_source_reads_it() {
    let detail = check_url("wss://api.example.com", &GRPC)
        .expect_err("a WebSocket URL is not a gRPC endpoint");

    assert!(
        detail.contains("`wss` scheme"),
        "names the scheme it saw: {detail}"
    );
    assert!(detail.contains("https://"), "names what to write: {detail}");
    assert!(
        detail.contains("INGEST_SOURCE=rpc"),
        "names the source that does read it: {detail}"
    );
}

#[test]
fn the_grpc_path_refuses_a_url_without_a_scheme_by_its_own_reason() {
    let detail = check_url("grpc.example.com:443", &GRPC)
        .expect_err("a schemeless URL is not a gRPC endpoint");

    assert!(detail.contains("no scheme"), "{detail}");
    assert!(
        !detail.contains("INGEST_SOURCE"),
        "no other source reads a URL with no scheme: {detail}"
    );
}
