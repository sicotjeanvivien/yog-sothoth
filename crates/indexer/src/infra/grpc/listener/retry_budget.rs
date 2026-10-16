//! Whether to try again after an attempt, and how long to wait first; the resume mark is
//! [`Verdict::next_resume_from`]'s, read from the same [`Verdict`].

use std::time::Duration;

use crate::error::GrpcListenerError;

use super::{ending::Verdict, log};

/// The same backoff as `SubscriptionWorker`: the same provider is on the other end.
/// ⚠️ A copy, not a shared definition: change both.
const INITIAL_BACKOFF_SECS: u64 = 1;
const MAX_BACKOFF_SECS: u64 = 60;

/// What the listener does after an attempt.
#[derive(Debug)]
pub(super) enum Next {
    /// Wait this long, then try again.
    Retry { after: Duration },
    /// Stop, with this result.
    Stop(Result<(), GrpcListenerError>),
}

/// How many failing attempts in a row the listener has made, and how long the
/// next one waits.
pub(super) struct RetryBudget {
    attempt: u32,
    backoff: u64,
    max_attempts: u32,
}

impl RetryBudget {
    pub(super) fn new(max_attempts: u32) -> Self {
        Self {
            attempt: 0,
            backoff: INITIAL_BACKOFF_SECS,
            max_attempts,
        }
    }

    /// Count one attempt and decide what follows its `verdict`; `resume_from` is only logged.
    /// ⚠️ A stream that delivered is churn whatever ended it, a GOAWAY being ordinary; one that
    /// delivered nothing is charged, or an exhausted quota would redial a billed provider for ever.
    pub(super) fn settle(&mut self, verdict: Verdict, resume_from: Option<u64>) -> Next {
        self.attempt += 1;

        match verdict {
            Verdict::ShutdownRequested => {
                log::stopping_on_shutdown();
                Next::Stop(Ok(()))
            }
            Verdict::ConsumerGone => {
                log::stopping_consumer_gone();
                Next::Stop(Ok(()))
            }
            Verdict::Delivered { error, .. } => {
                log::resubscribing(self.attempt, error.as_deref(), resume_from);
                self.churn()
            }
            Verdict::Refused { error } | Verdict::Unanswered { error } => {
                log::attempt_failed(self.attempt, self.max_attempts, &error, resume_from);
                self.charge(error)
            }
        }
    }

    /// The budget starts over, and so does the backoff.
    fn churn(&mut self) -> Next {
        self.attempt = 0;
        self.backoff = INITIAL_BACKOFF_SECS;
        Next::Retry {
            after: Duration::from_secs(INITIAL_BACKOFF_SECS),
        }
    }

    /// One failing attempt more: stop if it was the last, otherwise wait the
    /// backoff and double it.
    fn charge(&mut self, last_error: String) -> Next {
        if self.attempt >= self.max_attempts {
            return Next::Stop(Err(GrpcListenerError::RetriesExhausted {
                attempts: self.attempt,
                last_error,
            }));
        }

        let after = Duration::from_secs(self.backoff);
        self.backoff = (self.backoff * 2).min(MAX_BACKOFF_SECS);
        Next::Retry { after }
    }
}

#[cfg(test)]
#[path = "../tests/retry_budget_tests.rs"]
mod tests;
