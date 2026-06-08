use solana_pubkey::Pubkey;
use thiserror::Error;
use yog_core::domain::Protocol;

/// Errors returned by a `SubscriptionWorker` at end of life.
///
/// A worker only ever terminates with an error when it has exhausted its
/// own retry budget — all other exits (shutdown, dispatcher closed) are
/// `Ok(())`. This keeps the listener's decision trivial: an `Err` from
/// a worker means "this target is dead".
#[derive(Debug, Error, Clone)]
pub(crate) enum SubscriptionWorkerError {
    #[error("worker for {protocol} / {mention} gave up after {attempts} attempts: {last_error}")]
    RetriesExhausted {
        protocol: Protocol,
        mention: Pubkey,
        attempts: u32,
        last_error: String,
    },
}
