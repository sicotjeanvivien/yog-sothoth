use super::*;

fn ends_with(result: Result<(), SourceError>) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move { result })
}

fn semaphore_closed(stage: &'static str) -> SourceError {
    SourceError::SemaphoreClosed { stage }
}

/// ⚠️ **A stage that fails while the pipeline winds down must still be the
/// source's verdict.** The `select!` above answers with whichever stage ended
/// first, and on an orderly stop that is usually a clean `Ok(())` from the
/// dispatcher; a listener that panics a millisecond later reports only here.
/// Dropping what the drain collects would leave that failure with no error, no
/// exit code and no log line — the same defect `Daemon::run`'s `Stop` exists to
/// prevent one level up, and one that reached `main` once already.
///
/// Verified by mutation: put the `join` back behind a `let _` and this test is
/// the one that fails.
#[tokio::test]
async fn a_stage_that_fails_during_the_drain_is_the_verdict() {
    let (mut listener, mut dispatcher, mut fetch) = (
        ends_with(Err(semaphore_closed("listener"))),
        ends_with(Ok(())),
        ends_with(Ok(())),
    );

    let outcome = drain_stages(
        DISPATCHER,
        Ok(()),
        [
            (LISTENER, &mut listener),
            (DISPATCHER, &mut dispatcher),
            (FETCH, &mut fetch),
        ],
    )
    .await;

    let refused = outcome.expect_err("a stage that failed must not report success");
    assert!(
        matches!(refused, SourceError::SemaphoreClosed { stage: "listener" }),
        "the verdict must be the failure the drain found, got {refused:?}"
    );
}

/// And the other direction: the stage that ended first is the cause, and the
/// stages it brought down with it — all reporting `Ok(())` — must not replace
/// it. That is the whole reason the `select!` is `biased`.
#[tokio::test]
async fn the_stages_brought_down_by_a_failure_do_not_replace_it() {
    let (mut listener, mut dispatcher, mut fetch) =
        (ends_with(Ok(())), ends_with(Ok(())), ends_with(Ok(())));

    let outcome = drain_stages(
        LISTENER,
        Err(semaphore_closed("the cause")),
        [
            (LISTENER, &mut listener),
            (DISPATCHER, &mut dispatcher),
            (FETCH, &mut fetch),
        ],
    )
    .await;

    let refused = outcome.expect_err("the first failure stays the verdict");
    assert!(
        matches!(refused, SourceError::SemaphoreClosed { stage: "the cause" }),
        "got {refused:?}"
    );
}

/// ⚠️ **The stage that already answered is stepped over, and nothing else is.**
/// `tokio` panics on a `JoinHandle` polled after completion, so a drain that
/// forgot `ended` would take the whole source down on every ordinary stop —
/// loudly, but only at runtime, and only on the path with no test. Here the
/// listener's handle is deliberately left joined beforehand.
#[tokio::test]
async fn the_stage_that_already_answered_is_not_polled_again() {
    let mut listener = ends_with(Ok(()));
    let first = join(LISTENER, (&mut listener).await);
    let (mut dispatcher, mut fetch) = (ends_with(Ok(())), ends_with(Ok(())));

    let outcome = drain_stages(
        LISTENER,
        first,
        [
            (LISTENER, &mut listener),
            (DISPATCHER, &mut dispatcher),
            (FETCH, &mut fetch),
        ],
    )
    .await;

    assert!(outcome.is_ok());
}
