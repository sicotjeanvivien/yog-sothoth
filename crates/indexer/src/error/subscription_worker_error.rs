use solana_pubkey::Pubkey;
use thiserror::Error;
use yog_core::domain::Protocol;

/// A `SubscriptionWorker`'s only error, an exhausted retry budget; every other exit is `Ok(())`.
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
