use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> Arc<RpcListener> {
    Arc::new(RpcListener::new(
        Endpoint::for_tests(url, None),
        1,
        IngestScope::Pools,
    ))
}

/// ⚠️ The accept set, and it is two schemes rather than one on purpose:
/// `ws://` is what a local validator speaks, and narrowing this to `wss` would
/// break every local setup while every remote one stayed green.
#[test]
fn a_websocket_endpoint_is_accepted() {
    for url in ["wss://api.example.com", "ws://127.0.0.1:8900", "WSS://x.io"] {
        assert!(
            listener(url).check_scheme().is_ok(),
            "{url} is a WebSocket endpoint"
        );
    }
}

/// The failure this guard exists to remove, and the message is the whole point
/// of refusing here: `INGEST_STREAM_URL` is read by **both** sources, so the
/// operator who tried Yellowstone and came back keeps an `https://` URL.
/// Without the refusal that is `RPC_WORKER_MAX_RETRIES` attempts with backoff
/// *per watched pool*, then `AllWorkersGaveUp` — a configuration fault dressed
/// as an unreachable provider.
#[test]
fn a_grpc_endpoint_is_refused_and_says_which_source_reads_it() {
    let error = listener("https://grpc.example.com:443")
        .check_scheme()
        .expect_err("an https endpoint is not a WebSocket");

    let detail = error.to_string();
    assert!(
        detail.contains("https"),
        "names the scheme it saw: {detail}"
    );
    assert!(detail.contains("wss://"), "names what to write: {detail}");
    assert!(
        detail.contains("INGEST_SOURCE=grpc"),
        "names the source that does read it: {detail}"
    );
}

/// A distinct refusal, because it is a distinct mistake: telling an operator
/// their endpoint "carries the `` scheme" reads as a bug in the message.
#[test]
fn a_url_without_a_scheme_is_refused_by_its_own_reason() {
    let detail = listener("api.example.com:443")
        .check_scheme()
        .expect_err("a schemeless URL is not a WebSocket")
        .to_string();

    assert!(detail.contains("no scheme"), "{detail}");
}

/// ⚠️ **That `run` calls the guard is the thing worth testing**, and it is a
/// separate statement from the guard being right — deleting the call leaves
/// every test above green. Verified by mutation: `run` returns the refusal
/// before it dials anything, so this test needs no network.
#[tokio::test]
async fn run_refuses_the_wrong_scheme_before_touching_the_network() {
    let (tx, _rx) = mpsc::channel(1);

    let error = listener("https://grpc.example.com:443")
        .run(tx, CancellationToken::new())
        .await
        .expect_err("run must refuse before spawning a fleet");

    assert!(
        matches!(error, RpcListenerError::InvalidEndpoint { .. }),
        "got {error:?}"
    );
}
