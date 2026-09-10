use solana_pubkey::Pubkey;

use crate::{
    CoreResult,
    application::extraction::{ExtractionOutcome, OnChainTransaction},
};

/// Common interface for all supported AMM protocols.
///
/// Each protocol implements this trait. The indexer dispatches incoming
/// transactions to the correct implementation based on `program_id()`.
///
/// The transaction arrives as a [`OnChainTransaction`] — the neutral shape every
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
    fn extract_events(&self, tx: &OnChainTransaction) -> CoreResult<ExtractionOutcome>;

    /// Whether this extractor actually extracts anything.
    ///
    /// ⚠️ **A stub must say so, because subscribing to it costs money.** The
    /// ingestion decides what to watch from
    /// [`ExtractionDispatcher::implemented_protocols`], and watching a protocol
    /// means a program-wide subscription: on the JSON-RPC path, one
    /// `getTransaction` per transaction against a rate-limited quota; on the
    /// gRPC path, bandwidth billed by the byte. Doing that for an extractor
    /// that returns an empty outcome is paying a firehose to decode and
    /// discard.
    ///
    /// The default is `true` — an extractor extracts. A stub overrides it, and
    /// **the override disappears with the stub**: nobody has to remember a
    /// second list when the protocol is finally written, which is the failure
    /// this repository keeps paying for.
    ///
    /// [`ExtractionDispatcher::implemented_protocols`]: crate::application::extraction::ExtractionDispatcher::implemented_protocols
    fn is_implemented(&self) -> bool {
        true
    }
}
