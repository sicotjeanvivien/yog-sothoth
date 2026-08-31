use solana_pubkey::Pubkey;

use crate::{
    CoreResult,
    application::extraction::{ExtractionOutcome, TransactionView},
};

/// Common interface for all supported AMM protocols.
///
/// Each protocol implements this trait. The indexer dispatches incoming
/// transactions to the correct implementation based on `program_id()`.
///
/// The transaction arrives as a [`TransactionView`] — the neutral shape every
/// source adapter produces — so no implementation names a transport.
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
    /// Program ID this indexer handles.
    fn program_id(&self) -> Pubkey;

    /// Extract every domain event the transaction emitted for this protocol.
    fn extract_events(&self, tx: &TransactionView) -> CoreResult<ExtractionOutcome>;
}
