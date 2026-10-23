use super::*;

use yog_bootstrap::Endpoint;

fn listener(url: &str) -> Arc<RpcListener> {
    Arc::new(RpcListener::new(Endpoint::for_tests(url, None), 1))
}

/// That `run` calls the scheme check, not that the check is right (`scheme_tests`'s).
/// Mutation this is written against: deleting the call; it refuses before dialling, no network.
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

/// Mutation this is written against: `watch` inserting anything but the program id, which would
/// subscribe and hear nothing.
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

// ── Joining the fleet ─────────────────────────────────────────────────

fn target(n: u8) -> SubscriptionTarget {
    SubscriptionTarget::new(Protocol::MeteoraDammV2, Pubkey::new_from_array([n; 32]))
}

fn spawn_worker<F>(n: u8, body: F) -> WorkerHandle
where
    F: Future<Output = Result<(), SubscriptionWorkerError>> + Send + 'static,
{
    WorkerHandle {
        target: target(n),
        handle: tokio::spawn(body),
    }
}

/// ⚠️ Nothing aborts a worker: a cancelled `JoinError` is the runtime torn down, not a give-up.
/// Mutation this is written against: counting it in `gave_up`; `run`'s `is_cancelled` guard is
/// not on this path, so deleting it leaves this test green.
#[tokio::test]
async fn a_cancelled_worker_has_not_given_up() {
    let fleet: Vec<_> = [1, 2]
        .map(|n| spawn_worker(n, std::future::pending()))
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

/// With nothing in `gave_up` the verdict is `Ok`, so a stop that cancels every worker is no dead ingestion.
#[test]
fn a_fleet_that_gave_up_on_nothing_is_not_a_dead_ingestion() {
    assert!(fleet_outcome(&[], 2).is_ok());
}

/// `AllWorkersGaveUp` is reachable, and says what each worker reported: it is the last line the
/// process writes.
#[test]
fn a_fleet_that_gave_up_on_every_target_names_every_one_of_them() {
    let failures: Vec<_> = [1, 2]
        .map(|n| WorkerFailure {
            protocol: Protocol::MeteoraDammV2,
            mention: target(n).mention,
            reason: format!("retries_exhausted after 7: refused {n}"),
        })
        .into_iter()
        .collect();

    let Err(RpcListenerError::AllWorkersGaveUp { failures: said }) = fleet_outcome(&failures, 2)
    else {
        panic!("a fleet entirely out of retries is a dead ingestion");
    };

    for n in [1, 2] {
        assert!(
            said.contains(&target(n).mention.to_string()),
            "the message must name the target that died, got: {said}"
        );
        assert!(
            said.contains(&format!("refused {n}")),
            "the message must carry each worker's own reason, got: {said}"
        );
    }
    assert!(
        said.contains("meteora_damm_v2"),
        "the message must name the protocol, got: {said}"
    );
}

/// A worker whose future panicked is the one `JoinError` that is a failure, and stays counted.
#[tokio::test]
async fn a_panicking_worker_is_counted_as_a_panic() {
    let mut gave_up = Vec::new();

    join_fleet(
        vec![spawn_worker(1, async { panic!("boom") })],
        &mut gave_up,
    )
    .await;

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

/// A worker that returned `Err` spent its retry budget: counted, and says so.
#[tokio::test]
async fn a_worker_out_of_retries_is_counted_for_that_reason() {
    let mut gave_up = Vec::new();
    let exhausted = SubscriptionWorkerError::RetriesExhausted {
        protocol: Protocol::MeteoraDammV2,
        mention: target(1).mention,
        attempts: 7,
        last_error: "connection refused".to_string(),
    };

    join_fleet(
        vec![spawn_worker(1, async move { Err(exhausted) })],
        &mut gave_up,
    )
    .await;

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

    join_fleet(vec![spawn_worker(1, async { Ok(()) })], &mut gave_up).await;

    assert!(gave_up.is_empty(), "got {gave_up:?}");
}
