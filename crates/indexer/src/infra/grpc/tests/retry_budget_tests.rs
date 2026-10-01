//! Tests for the retry budget, on the values it returns — no clock involved.

use super::*;

const MAX: u32 = 5;

fn failed_empty() -> Attempt {
    Attempt::Failed {
        error: "refused".to_string(),
        delivered: false,
        resume_from: None,
    }
}

fn broke_after_delivering() -> Attempt {
    Attempt::Failed {
        error: "reset".to_string(),
        delivered: true,
        resume_from: Some(8),
    }
}

/// One attempt, settled.
fn attempt(budget: &mut RetryBudget, outcome: Attempt) -> Next {
    budget.start_attempt();
    budget.settle(outcome, None)
}

fn waits(next: Next) -> Duration {
    match next {
        Next::Retry { after } => after,
        Next::Stop(result) => panic!("expected a retry, got a stop: {result:?}"),
    }
}

/// ⚠️ **A churn resets the backoff**, not only the attempt count. Without it a
/// stream that broke after a run of failures would wait the doubled backoff of
/// those failures before its first redial.
///
/// Mutation this is written against: `churn` leaving `backoff` as it was.
#[test]
fn a_churn_resets_the_backoff() {
    let mut budget = RetryBudget::new(MAX);

    assert_eq!(
        waits(attempt(&mut budget, failed_empty())),
        Duration::from_secs(1)
    );
    assert_eq!(
        waits(attempt(&mut budget, failed_empty())),
        Duration::from_secs(2)
    );
    assert_eq!(
        waits(attempt(&mut budget, broke_after_delivering())),
        Duration::from_secs(1),
        "a churn redials after one second"
    );

    assert_eq!(
        waits(attempt(&mut budget, failed_empty())),
        Duration::from_secs(INITIAL_BACKOFF_SECS),
        "the first failure after a churn waits the initial backoff, not the doubled one"
    );
}

/// The budget runs out on the `max`-th failing attempt in a row.
#[test]
fn the_budget_runs_out_after_max_attempts() {
    let mut budget = RetryBudget::new(MAX);

    for _ in 1..MAX {
        waits(attempt(&mut budget, failed_empty()));
    }

    match attempt(&mut budget, failed_empty()) {
        Next::Stop(Err(GrpcListenerError::RetriesExhausted {
            attempts,
            last_error,
        })) => {
            assert_eq!(attempts, MAX);
            assert_eq!(last_error, "refused");
        }
        other => panic!("the {MAX}th failure in a row must stop: {other:?}"),
    }
}

/// A churn restarts the count: `max` more failures are needed after it.
#[test]
fn a_churn_restarts_the_count() {
    let mut budget = RetryBudget::new(MAX);

    for _ in 1..MAX {
        waits(attempt(&mut budget, failed_empty()));
    }
    waits(attempt(&mut budget, broke_after_delivering()));

    for _ in 1..MAX {
        waits(attempt(&mut budget, failed_empty()));
    }
    assert!(
        matches!(attempt(&mut budget, failed_empty()), Next::Stop(Err(_))),
        "the count starts over at the churn"
    );
}

/// The backoff doubles and stops at its ceiling.
#[test]
fn the_backoff_is_capped() {
    let mut budget = RetryBudget::new(u32::MAX);

    let waited: Vec<u64> = (0..9)
        .map(|_| waits(attempt(&mut budget, failed_empty())).as_secs())
        .collect();

    assert_eq!(waited, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
}

/// A requested stop and a vanished consumer both end the run cleanly.
#[test]
fn a_shutdown_and_a_vanished_consumer_stop_cleanly() {
    let mut budget = RetryBudget::new(MAX);

    assert!(matches!(
        attempt(&mut budget, Attempt::ShutdownRequested),
        Next::Stop(Ok(()))
    ));
    assert!(matches!(
        attempt(&mut budget, Attempt::DownstreamClosed),
        Next::Stop(Ok(()))
    ));
}
