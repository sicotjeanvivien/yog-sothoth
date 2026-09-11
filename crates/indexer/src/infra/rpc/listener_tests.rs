use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> Arc<RpcListener> {
    Arc::new(RpcListener::new(
        Endpoint::for_tests(url, None),
        1,
        IngestScope::Pools,
    ))
}

/// ⚠️ **That `run` calls the scheme check is the thing worth testing here**, and
/// it is a separate statement from the check being right — which is
/// `scheme_tests`'s. Deleting the call leaves every test there green. Verified
/// by mutation: `run` returns the refusal before it dials anything, so this
/// test needs no network.
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
