use super::*;

#[test]
fn handle_task_result_clean_stop_returns_ok() {
    let result: Result<Result<(), std::io::Error>, tokio::task::JoinError> = Ok(Ok(()));
    assert!(handle_task_result(result, "test task").is_ok());
}

#[test]
fn handle_task_result_task_error_returns_err() {
    let err = std::io::Error::other("boom");
    let result: Result<Result<(), std::io::Error>, tokio::task::JoinError> = Ok(Err(err));
    assert!(handle_task_result(result, "test task").is_err());
}

// ── index_concurrency ────────────────────────────────────────────────────────
//
// The guard below cannot fire in this binary — `init_db` goes through
// `Database::connect`, whose pool is fixed at ten. That is exactly why it is
// tested here: a refusal no configuration can reach today is a refusal nobody
// would notice was broken, and the one it replaced (`.max(1)`) was broken in
// precisely the case it claimed to cover.

#[test]
fn a_pool_leaves_one_connection_for_the_reporter() {
    assert_eq!(index_concurrency(10).unwrap(), 9);
    assert_eq!(index_concurrency(2).unwrap(), 1);
}

#[test]
fn a_pool_with_nothing_left_to_reserve_is_refused() {
    // One connection is the case the previous version got wrong: it clamped to
    // 1, handing the reporter's only connection to an index task.
    let refusal = index_concurrency(1).unwrap_err().to_string();
    assert!(
        refusal.contains("1 connection(s)") && refusal.contains("more connections"),
        "the refusal must name the pool it saw and what to do about it, got: {refusal}"
    );

    assert!(index_concurrency(0).is_err());
}

#[test]
fn the_bound_never_reaches_zero() {
    // `Semaphore::new(0)` would deadlock rather than fail, so no accepted pool
    // size may produce it. Checked across the whole range the guard admits.
    for max_connections in 2..64 {
        assert!(
            index_concurrency(max_connections).unwrap() >= 1,
            "pool of {max_connections} produced a zero bound"
        );
    }
}

/// ⚠️ **A cancellation is not a panic, and this is the daemon's half of that
/// statement** — `infra::rpc::listener` holds the other. Nothing aborts these
/// three tasks, so a cancelled `JoinError` here means the runtime was torn down
/// around one: the work was cut short, it did not fail, and reporting it as a
/// panic on every ordinary stop is what buried the real ones.
#[tokio::test]
async fn a_cancelled_task_is_not_reported_as_a_panic() {
    let handle = tokio::spawn(std::future::pending::<Result<(), std::io::Error>>());
    handle.abort();
    let cancelled = handle.await;
    assert!(
        cancelled
            .as_ref()
            .is_err_and(tokio::task::JoinError::is_cancelled),
        "the fixture must produce a cancelled JoinError, not something else"
    );

    assert!(
        handle_task_result(cancelled, "test task").is_ok(),
        "a task destroyed before it could answer has not failed"
    );
}

/// The other reason, exercised on its own: a task whose future panicked is a
/// failure, and stays one.
#[tokio::test]
async fn a_panicking_task_is_still_an_error() {
    let handle = tokio::spawn(async { panic!("boom") });
    let panicked: Result<Result<(), std::io::Error>, _> = handle.await;
    assert!(
        panicked.as_ref().is_err_and(|e| e.is_panic()),
        "the fixture must produce a panic, not a cancellation"
    );

    assert!(handle_task_result(panicked, "test task").is_err());
}

// ── Stop ────────────────────────────────────────────────────────────────────

fn ends_with<E: std::error::Error + Send + Sync + 'static>(
    result: Result<(), E>,
) -> JoinHandle<Result<(), E>> {
    tokio::spawn(async move { result })
}

fn far_enough_off() -> Instant {
    Instant::now() + Duration::from_secs(30)
}

#[tokio::test]
async fn a_task_that_ends_in_time_is_not_reported_as_still_running() {
    let mut stop = Stop::new(Ok(()));

    stop.settle(
        "test task",
        &mut ends_with(Ok::<(), std::io::Error>(())),
        far_enough_off(),
    )
    .await;

    assert!(stop.still_running.is_empty());
    assert!(stop.finish().is_ok());
}

/// ⚠️ **This is the test the grace exists for, and the one that proves it is
/// wired.** `settle` must come back from a task that never will, and say which
/// one — the name it keeps is what the `warn!` prints, so a stage that overran
/// can be read in the logs instead of guessed at.
///
/// Verified by mutation: replace the `timeout_at` in `settle` with a bare
/// `handle.await` and this test **hangs** rather than failing — there is no
/// assertion that could catch a missing bound, only the clock.
#[tokio::test(start_paused = true)]
async fn a_task_that_outlives_the_grace_is_named() {
    let mut stop = Stop::new(Ok(()));

    stop.settle(
        "test task",
        &mut tokio::spawn(std::future::pending::<Result<(), std::io::Error>>()),
        far_enough_off(),
    )
    .await;

    assert_eq!(stop.still_running, vec!["test task"]);
}

/// ⚠️ **A stage that failed can report after the `select!` has already
/// answered, and its error must still be the verdict.**
/// `RpcTransactionSource::run` cancels the shared token before returning, so on
/// a dead ingestion the daemon's cancellation arm fires first with its
/// verdict-less `Ok(())` and `AllWorkersGaveUp` only arrives here. Measured on
/// 14 September 2026 against a dead endpoint: the process logged
/// `transaction source failed` and **exited 0**, where the same run on the
/// previous revision exited 1. A supervisor keyed on the exit code would have
/// left a dead ingestion running.
#[tokio::test]
async fn an_error_reported_after_the_verdict_becomes_the_verdict() {
    let mut stop = Stop::new(Ok(()));

    stop.settle(
        "test task",
        &mut ends_with(Err(std::io::Error::other("all workers gave up"))),
        far_enough_off(),
    )
    .await;

    let refused = stop.finish().expect_err("a failed stage must not exit 0");
    assert!(
        refused.to_string().contains("all workers gave up"),
        "the verdict must carry the failure it found, got: {refused}"
    );
}

/// And the other direction, which is what "first error wins" costs if it is
/// written the lazy way: the stages that stop cleanly *after* a failure must
/// not erase it. The drain always finds some of those — a dead source stops the
/// indexer and the reporter, and both exit `Ok`.
#[tokio::test]
async fn a_clean_stop_after_a_failure_does_not_erase_it() {
    let mut stop = Stop::new(Err(anyhow::anyhow!("the source failed first")));

    stop.settle(
        "test task",
        &mut ends_with(Ok::<(), std::io::Error>(())),
        far_enough_off(),
    )
    .await;

    let refused = stop.finish().expect_err("the first failure is the verdict");
    assert!(
        refused.to_string().contains("the source failed first"),
        "got: {refused}"
    );
}
