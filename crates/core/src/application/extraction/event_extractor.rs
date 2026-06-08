// use solana_transaction_status::EncodedConfirmedTransactionWithStatusMeta;

use solana_transaction_status_client_types::EncodedConfirmedTransactionWithStatusMeta;

use crate::{CoreResult, application::extraction::ExtractionOutcome};

/// Common interface for all supported AMM protocols.
///
/// Each protocol implements this trait. The indexer dispatches incoming
/// transactions to the correct implementation based on `program_id()`.
///
/// # Contract
///
/// `extract_events` is the single entry point. It walks the transaction,
/// decodes every protocol-specific event it can, translates them into
/// protocol-agnostic [`crate::domain::DomainEvent`] variants, and returns
/// an [`ExtractionOutcome`] that ventilates successes / unknowns / failures.
///
/// The implementation MUST NOT panic on partial failures (unrecognized
/// discriminators, borsh errors, missing transferChecked context, etc.).
/// Those go into `unknown` or `failures`. A returned `Err` is reserved
/// for transaction-level malformations (no log messages, no inner
/// instructions when they were required, etc.).
pub trait EventExtractor: Send + Sync {
    /// Program ID this indexer handles, as base58 string.
    fn program_id(&self) -> &str;

    /// Extract every domain event the transaction emitted for this protocol.
    fn extract_events(
        &self,
        tx: &EncodedConfirmedTransactionWithStatusMeta,
    ) -> CoreResult<ExtractionOutcome>;
}
