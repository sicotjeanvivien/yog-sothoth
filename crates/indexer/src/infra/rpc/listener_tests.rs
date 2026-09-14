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

// ── Joining the fleet ────────────────────────────────────────────────────────

fn target(n: u8) -> SubscriptionTarget {
    SubscriptionTarget::new(Protocol::MeteoraDammV2, Pubkey::new_from_array([n; 32]))
}

fn worker<F>(n: u8, body: F) -> WorkerHandle
where
    F: Future<Output = Result<(), SubscriptionWorkerError>> + Send + 'static,
{
    WorkerHandle {
        target: target(n),
        handle: tokio::spawn(body),
    }
}

/// ⚠️ **The behaviour this test holds used to be held by the guard below it.**
/// Nothing aborts a worker, so a cancelled `JoinError` means the runtime was
/// torn down around the fleet — an interrupted stop, not a fleet that gave up.
/// Counting it as an abandonment filled `gave_up` to the brim on every Ctrl-C,
/// and `run`'s `if shutdown.is_cancelled()` was all that stood between that and
/// `AllWorkersGaveUp`.
///
/// Verified by mutation, which is the whole point of splitting it out: delete
/// that `if` from `run` and this test, with the one below it, stays green.
/// Neither goes through it.
#[tokio::test]
async fn a_cancelled_worker_has_not_given_up() {
    let fleet: Vec<_> = [1, 2]
        .map(|n| worker(n, std::future::pending()))
        .into_iter()
        .collect();
    for h in &fleet {
        h.handle.abort();
    }

    let mut gave_up = Vec::new();
    join_fleet(fleet, &mut gave_up).await;

    assert!(
        gave_up.is_empty(),
        "an interrupted stop is not an abandonment, got {gave_up:?}"
    );
}

/// The companion of the test above: with nothing in `gave_up`, the fleet's
/// verdict is `Ok` — so a stop that cancels every worker cannot be read as a
/// dead ingestion even once the guard in `run` is gone.
#[test]
fn a_fleet_that_gave_up_on_nothing_is_not_a_dead_ingestion() {
    assert!(fleet_outcome(&[], 2).is_ok());
}

/// And the reason `AllWorkersGaveUp` exists at all, so the test above cannot
/// pass by the verdict having become unreachable.
#[test]
fn a_fleet_that_gave_up_on_every_target_is_a_dead_ingestion() {
    let failures: Vec<_> = [1, 2]
        .map(|n| WorkerFailure {
            protocol: Protocol::MeteoraDammV2,
            mention: target(n).mention,
            reason: "retries_exhausted".to_string(),
        })
        .into_iter()
        .collect();

    assert!(matches!(
        fleet_outcome(&failures, 2),
        Err(RpcListenerError::AllWorkersGaveUp { .. })
    ));
}

/// A worker whose future panicked is the one `JoinError` that *is* a failure,
/// and it stays counted. Only this case triggers the panic branch: the
/// cancellation test above never panics, and this one never cancels.
#[tokio::test]
async fn a_panicking_worker_is_counted_as_a_panic() {
    let mut gave_up = Vec::new();

    join_fleet(vec![worker(1, async { panic!("boom") })], &mut gave_up).await;

    let [failure] = gave_up.as_slice() else {
        panic!("a panic must be counted, got {gave_up:?}");
    };
    assert_eq!(failure.mention, target(1).mention);
    assert!(
        failure.reason.starts_with("panic:"),
        "the reason must name the panic, got {}",
        failure.reason
    );
}

/// The third reason, on its own: a worker that returned `Err` spent its whole
/// retry budget. It is counted for a different cause than a panic, and says so.
#[tokio::test]
async fn a_worker_out_of_retries_is_counted_for_that_reason() {
    let mut gave_up = Vec::new();
    let exhausted = SubscriptionWorkerError::RetriesExhausted {
        protocol: Protocol::MeteoraDammV2,
        mention: target(1).mention,
        attempts: 7,
        last_error: "connection refused".to_string(),
    };

    join_fleet(vec![worker(1, async move { Err(exhausted) })], &mut gave_up).await;

    let [failure] = gave_up.as_slice() else {
        panic!("an exhausted budget must be counted, got {gave_up:?}");
    };
    assert!(
        failure.reason.starts_with("retries_exhausted after 7:"),
        "the reason must name the budget it spent, got {}",
        failure.reason
    );
}

/// A worker that stopped when asked is neither, and leaves nothing behind.
#[tokio::test]
async fn a_worker_that_stopped_cleanly_leaves_nothing_behind() {
    let mut gave_up = Vec::new();

    join_fleet(vec![worker(1, async { Ok(()) })], &mut gave_up).await;

    assert!(gave_up.is_empty(), "got {gave_up:?}");
}
