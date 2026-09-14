use super::*;

fn ends_with<E: std::error::Error + Send + Sync + 'static>(
    result: Result<(), E>,
) -> JoinHandle<Result<(), E>> {
    tokio::spawn(async move { result })
}

// ── TaskEnd ──────────────────────────────────────────────────────────────────
//
// Read directly by callers that join without `handle_task_result` — the RPC
// fleet's join loop and `yog-signals`' `JoinSet` drain — so the classification
// is exercised on its own and not only through the helper.

/// One case per reason, and each exercises only its own: the fixture is
/// checked with `tokio`'s own predicate before the assertion, so a test that
/// produced the *other* kind of `JoinError` fails on the fixture rather than
/// quietly asserting nothing.
#[tokio::test]
async fn an_aborted_task_reads_as_cancelled() {
    let handle = tokio::spawn(std::future::pending::<()>());
    handle.abort();
    let error = handle
        .await
        .expect_err("an aborted task must not join cleanly");
    assert!(error.is_cancelled(), "the fixture must be a cancellation");

    assert_eq!(TaskEnd::from(&error), TaskEnd::Cancelled);
}

#[tokio::test]
async fn a_panicking_task_reads_as_panicked() {
    let error = tokio::spawn(async { panic!("boom") })
        .await
        .expect_err("a panicking task must not join cleanly");
    assert!(error.is_panic(), "the fixture must be a panic");

    assert_eq!(TaskEnd::from(&error), TaskEnd::Panicked);
}

// ── handle_task_result ───────────────────────────────────────────────────────

#[test]
fn a_clean_stop_returns_ok() {
    let result: Result<Result<(), std::io::Error>, JoinError> = Ok(Ok(()));
    assert!(handle_task_result(result, "test task").is_ok());
}

#[test]
fn a_task_error_returns_err() {
    let err = std::io::Error::other("boom");
    let result: Result<Result<(), std::io::Error>, JoinError> = Ok(Err(err));
    assert!(handle_task_result(result, "test task").is_err());
}

/// ⚠️ **A cancellation is not a panic**, and reporting it as one is what buried
/// the real panics: it put the most alarming word in the logs on the most
/// ordinary path. The fixture is checked before the assertion so it cannot lie
/// about which of the two reasons it produced.
#[tokio::test]
async fn a_cancelled_task_is_not_reported_as_a_panic() {
    let handle = tokio::spawn(std::future::pending::<Result<(), std::io::Error>>());
    handle.abort();
    let cancelled = handle.await;
    assert!(
        cancelled.as_ref().is_err_and(JoinError::is_cancelled),
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
        panicked.as_ref().is_err_and(JoinError::is_panic),
        "the fixture must produce a panic, not a cancellation"
    );

    assert!(handle_task_result(panicked, "test task").is_err());
}

// ── Stop ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_task_that_ends_in_time_is_not_reported_as_still_running() {
    let mut stop = Stop::new(Ok(()));

    stop.settle("test task", &mut ends_with(Ok::<(), std::io::Error>(())))
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
    )
    .await;

    assert_eq!(stop.still_running, vec!["test task"]);
}

/// ⚠️ **The grace is spent across the stages, not granted to each one.** Two
/// tasks that never end must cost one grace in total, or a daemon with three
/// stages would wait three times what its `warn!` claims — and, under Docker,
/// meet the SIGKILL that the value of [`SHUTDOWN_GRACE`] was chosen to stay
/// under. Both are named, and the second one is not waited for a second time.
///
/// Verified by mutation: recompute the deadline inside `settle`
/// (`Instant::now() + SHUTDOWN_GRACE`) and the elapsed assertion fails at twice
/// the grace, while the `still_running` one stays green.
///
/// ⚠️ **The equality is the assertion, not `< 2 ×`.** Under `start_paused` the
/// clock is driven by the timers alone, so a shared deadline costs *exactly*
/// one grace and nothing else is a rounding error. A bound of twice the grace
/// would have accepted every wrong answer strictly below it — one and a half
/// graces, say — and only caught the one the mutation above happens to produce.
#[tokio::test(start_paused = true)]
async fn two_stages_that_never_end_share_one_grace() {
    let started = Instant::now();
    let mut stop = Stop::new(Ok(()));

    for name in ["first stage", "second stage"] {
        stop.settle(
            name,
            &mut tokio::spawn(std::future::pending::<Result<(), std::io::Error>>()),
        )
        .await;
    }

    assert_eq!(stop.still_running, vec!["first stage", "second stage"]);
    assert_eq!(
        started.elapsed(),
        SHUTDOWN_GRACE,
        "two stages that never end must cost exactly one grace"
    );
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

    stop.settle("test task", &mut ends_with(Ok::<(), std::io::Error>(())))
        .await;

    let refused = stop.finish().expect_err("the first failure is the verdict");
    assert!(
        refused.to_string().contains("the source failed first"),
        "got: {refused}"
    );
}
