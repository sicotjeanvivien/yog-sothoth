//! What can be tested in a file whose header says it cannot be.
//!
//! The connection, the retry budget and `from_slot` need a server, and that is
//! `02 - backlog/pre-v02/flux-grpc-reel-mesures.md`. `channel_endpoint` is the
//! exception: it is a pure function of the configured endpoint, it runs
//! **before** the loop, and what it refuses is the misconfiguration this path
//! makes possible. What each refusal *says* is `scheme_tests`'s; what is tested
//! here is that this path builds its endpoint, and asks.

use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> GrpcListener {
    GrpcListener::new(Endpoint::for_tests(url, None), 1, IngestScope::Pools)
}

/// Accepted means built: TLS and keep-alive are configured on the way out, so
/// this also says a plaintext `http://` endpoint survives `tls_config`.
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

/// ⚠️ The defect this call was written for, and it was found by *reading a
/// successful-looking run*: the gRPC path was launched against the `wss://` URL
/// of `.env`, produced ten retries with backoff, and that was taken for "a
/// transport error, as expected". Deleting the scheme check from
/// `channel_endpoint` turns this red; `scheme_tests` would stay green.
#[test]
fn a_websocket_endpoint_is_refused_before_the_loop() {
    let error = listener("wss://api.example.com")
        .channel_endpoint()
        .expect_err("a WebSocket URL is not a gRPC endpoint");

    assert!(
        matches!(error, GrpcListenerError::InvalidEndpoint { .. }),
        "got {error:?}"
    );
}
