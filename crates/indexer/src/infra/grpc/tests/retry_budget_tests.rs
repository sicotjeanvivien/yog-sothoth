//! Tests for the retry budget's backoff, on its values; which ending charges it is `listener_tests`'.

use super::*;

const MAX: u32 = 5;

fn failed_empty() -> Verdict {
    Verdict::Refused {
        error: "refused".to_string(),
    }
}

fn broke_after_delivering() -> Verdict {
    Verdict::Delivered {
        mark: Some(8),
        error: Some("reset".to_string()),
    }
}

/// One attempt, settled.
fn attempt(budget: &mut RetryBudget, verdict: Verdict) -> Next {
    budget.settle(verdict, None)
}

fn waits(next: Next) -> Duration {
    match next {
        Next::Retry { after } => after,
        Next::Stop(result) => panic!("expected a retry, got a stop: {result:?}"),
    }
}

/// ⚠️ A churn resets the backoff too, or a stream broken after failures waits their doubled backoff.
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
        Duration::from_secs(INITIAL_BACKOFF_SECS),
        "a churn redials after the initial backoff"
    );

    assert_eq!(
        waits(attempt(&mut budget, failed_empty())),
        Duration::from_secs(INITIAL_BACKOFF_SECS),
        "the first failure after a churn waits the initial backoff, not the doubled one"
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
