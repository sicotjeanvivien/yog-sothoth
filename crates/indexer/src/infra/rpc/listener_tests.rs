use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> Arc<RpcListener> {
    Arc::new(RpcListener::new(Endpoint::for_tests(url, None), 1))
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

/// ⚠️ **A watched protocol is its program id, and this is the only place that
/// says so.** The listener holds one set of addresses and no scope, so the
/// translation lives in `watch` — and a `watch` that inserted anything else
/// would subscribe, connect, and hear nothing. A pool is taken as given.
#[tokio::test]
async fn a_protocol_is_watched_through_its_program_id_and_a_pool_as_itself() {
    let listener = listener("wss://api.example.com");
    let pool = Pubkey::new_from_array([7; 32]);

    listener.watch(Protocol::MeteoraDammV2).await;
    listener.watch_pool(Protocol::MeteoraDammV2, pool).await;

    let targets: HashSet<_> = listener
        .build_subscription_targets()
        .await
        .expect("two addresses are watched")
        .into_iter()
        .collect();

    assert_eq!(
        targets,
        HashSet::from([
            SubscriptionTarget::new(
                Protocol::MeteoraDammV2,
                Protocol::MeteoraDammV2.program_id()
            ),
            SubscriptionTarget::new(Protocol::MeteoraDammV2, pool),
        ])
    );
}
