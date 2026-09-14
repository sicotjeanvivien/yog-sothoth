use super::*;

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
