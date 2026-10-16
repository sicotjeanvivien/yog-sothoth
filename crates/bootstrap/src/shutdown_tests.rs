use super::*;

fn ends_with<E: std::error::Error + Send + Sync + 'static>(
    result: Result<(), E>,
) -> JoinHandle<Result<(), E>> {
    tokio::spawn(async move { result })
}

// ── TaskEnd ──────────────────────────────────────────────────────────────────
// Some callers join without `handle_task_result`: the classification is tested alone.

/// One case per reason.
/// ⚠️ Each fixture is checked with `tokio`'s predicate first, so it cannot produce the other kind.
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

/// Mutation: reading a cancellation as a panic, the most alarming word on the most
/// ordinary path.
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

/// A task whose future panicked is a failure.
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
    let mut stop = Stop::new(Ok(()), None);

    stop.settle("test task", &mut ends_with(Ok::<(), std::io::Error>(())))
        .await;

    assert!(stop.still_running.is_empty());
    assert!(stop.finish().is_ok());
}

/// Mutation: deleting the `collected` early return in `settle` (`polled after
/// completion`), or skipping every stage rather than the named one.
#[tokio::test(start_paused = true)]
async fn the_stage_the_select_already_collected_is_not_joined_again() {
    let mut spent = ends_with(Ok::<(), std::io::Error>(()));
    // What the daemon's `select!` arm did: this handle is polled to completion.
    assert!(
        (&mut spent).await.is_ok(),
        "the fixture must be a handle already joined, not a fresh one"
    );

    let mut stop = Stop::new(Ok(()), Some("spent stage"));
    stop.settle("spent stage", &mut spent).await;
    stop.settle(
        "other stage",
        &mut tokio::spawn(std::future::pending::<Result<(), std::io::Error>>()),
    )
    .await;

    assert_eq!(
        stop.still_running,
        vec!["other stage"],
        "only the stage that was never collected is drained, and it is drained normally"
    );
}

/// `settle` comes back from a task that never ends, and names it.
/// Mutation: `timeout_at` → a bare `handle.await`, which makes this test **hang**.
#[tokio::test(start_paused = true)]
async fn a_task_that_outlives_the_grace_is_named() {
    let mut stop = Stop::new(Ok(()), None);

    stop.settle(
        "test task",
        &mut tokio::spawn(std::future::pending::<Result<(), std::io::Error>>()),
    )
    .await;

    assert_eq!(stop.still_running, vec!["test task"]);
}

/// Two stages that never end cost one grace in total, and both are named.
/// Mutation: a deadline recomputed inside `settle`. ⚠️ Asserted by equality: the clock is paused.
#[tokio::test(start_paused = true)]
async fn two_stages_that_never_end_share_one_grace() {
    let started = Instant::now();
    let mut stop = Stop::new(Ok(()), None);

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

/// An error reported during the drain, after the cancellation arm's `Ok`, is the verdict.
/// Mutation: keeping the `select!`'s `Ok`, which exits 0 on a dead ingestion.
#[tokio::test]
async fn an_error_reported_after_the_verdict_becomes_the_verdict() {
    let mut stop = Stop::new(Ok(()), None);

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

/// Mutation: a stage stopping cleanly after a failure overwriting its error.
#[tokio::test]
async fn a_clean_stop_after_a_failure_does_not_erase_it() {
    let mut stop = Stop::new(Err(anyhow::anyhow!("the source failed first")), None);

    stop.settle("test task", &mut ends_with(Ok::<(), std::io::Error>(())))
        .await;

    let refused = stop.finish().expect_err("the first failure is the verdict");
    assert!(
        refused.to_string().contains("the source failed first"),
        "got: {refused}"
    );
}
