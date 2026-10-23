use super::*;

fn spawn_ending_with(result: Result<(), SourceError>) -> JoinHandle<Result<(), SourceError>> {
    tokio::spawn(async move { result })
}

fn semaphore_closed(stage: &'static str) -> SourceError {
    SourceError::SemaphoreClosed { stage }
}

/// ⚠️ A stage that fails while the pipeline winds down is still the source's verdict.
/// Mutation this is written against: the drain's `join` behind a `let _`.
#[tokio::test]
async fn a_stage_that_fails_during_the_drain_is_the_verdict() {
    let (mut listener, mut dispatcher, mut fetch) = (
        spawn_ending_with(Err(semaphore_closed("listener"))),
        spawn_ending_with(Ok(())),
        spawn_ending_with(Ok(())),
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

/// The stage that ended first is the cause; the stages it brought down, `Ok(())`, must not
/// replace it.
#[tokio::test]
async fn the_stages_brought_down_by_a_failure_do_not_replace_it() {
    let (mut listener, mut dispatcher, mut fetch) = (
        spawn_ending_with(Ok(())),
        spawn_ending_with(Ok(())),
        spawn_ending_with(Ok(())),
    );

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

/// ⚠️ tokio panics on a `JoinHandle` polled after completion: the drain must step over `ended`,
/// left joined here on purpose.
#[tokio::test]
async fn the_stage_that_already_answered_is_not_polled_again() {
    let mut listener = spawn_ending_with(Ok(()));
    let first = join(LISTENER, (&mut listener).await);
    let (mut dispatcher, mut fetch) = (spawn_ending_with(Ok(())), spawn_ending_with(Ok(())));

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
