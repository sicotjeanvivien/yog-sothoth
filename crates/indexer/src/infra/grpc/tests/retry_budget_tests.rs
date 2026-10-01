//! Tests for the retry budget, on the values it returns — no clock involved.
//!
//! Only what `run` cannot show: how long the next attempt waits. Which ending
//! restarts the budget and which charges it is owned by `listener_tests`, one
//! test per rule, and is not repeated here.

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
