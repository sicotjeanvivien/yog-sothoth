//! Whether to try again after an attempt, and how long to wait first.
//!
//! The other half of the retry rule — the resume mark — is
//! [`Attempt::next_resume_from`]'s, kept apart so that neither rule hides in
//! the other's branches.

use std::time::Duration;

use crate::error::GrpcListenerError;

use super::{ending::Attempt, log};

/// Retry budget shape, shared with `SubscriptionWorker` — the same provider is
/// on the other end, and an operator reading two different backoffs would have
/// to learn two.
pub(super) const INITIAL_BACKOFF_SECS: u64 = 1;
pub(super) const MAX_BACKOFF_SECS: u64 = 60;

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

    /// An attempt is about to start.
    pub(super) fn start_attempt(&mut self) {
        self.attempt += 1;
    }

    /// Decide what follows `outcome`. `resume_from` is the mark the next
    /// attempt will ask for, already decided — it is only logged here.
    pub(super) fn settle(&mut self, outcome: Attempt, resume_from: Option<u64>) -> Next {
        match outcome {
            Attempt::ShutdownRequested => {
                log::stopping_on_shutdown();
                Next::Stop(Ok(()))
            }
            Attempt::DownstreamClosed => {
                log::stopping_consumer_gone();
                Next::Stop(Ok(()))
            }

            // Lived long enough to deliver: churn, not a failing provider — the
            // same reading `SubscriptionWorker` makes of a closed stream.
            Attempt::StreamClosed {
                delivered: true, ..
            } => {
                log::resubscribing_after_close(self.attempt);
                self.churn()
            }

            // ⚠️ A delivered stream that errored or stalled is churn too: a
            // GOAWAY, an h2 reset or a nightly provider restart is how a
            // long-lived stream ordinarily breaks, and charging it would stop
            // the indexer on the tenth restart having lost nothing. The cost: a
            // provider that delivers and then fails every time is retried for
            // ever — `received_data` narrows it (a ping does not count).
            Attempt::Failed {
                error,
                delivered: true,
                ..
            }
            | Attempt::Stalled {
                error,
                delivered: true,
                ..
            } => {
                log::resubscribing_after_break(self.attempt, &error, resume_from);
                self.churn()
            }

            // ⚠️ A stream that opened and closed having delivered nothing is a
            // failing attempt, not churn: an exhausted quota, a `from_slot`
            // past retention, a token refused at stream level all look like
            // this, and resetting on them redials a billed provider for ever.
            Attempt::StreamClosed {
                delivered: false, ..
            } => {
                log::closed_empty(self.attempt, self.max_attempts);
                self.charge("stream closed without delivering anything".to_string())
            }

            // Endings that reached nothing, were refused, or met silence cost
            // the same attempt. What they do to the mark differs, and is
            // `Attempt::next_resume_from`'s.
            Attempt::Unreachable { error }
            | Attempt::Failed {
                error,
                delivered: false,
                ..
            }
            | Attempt::Stalled {
                error,
                delivered: false,
                ..
            } => {
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
            after: Duration::from_secs(1),
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
