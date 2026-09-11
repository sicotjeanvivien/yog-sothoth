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
    /// ⚠️ **No default, deliberately.** It had one — `true` — for the length of
    /// a review, and a default here points the wrong way: the expensive answer
    /// would be the one a new extractor gives by saying nothing. A protocol
    /// added by following the `add-protocol` recipe with a stub body, which is
    /// exactly how `MeteoraDlmm` exists today, would compile, keep every test
    /// green, and subscribe the indexer to a program id on the next restart.
    ///
    /// Required, the compiler asks the question at the one moment somebody can
    /// answer it — while writing the extractor and knowing whether it extracts.
    /// It costs each implementation one line and it is a spending decision, not
    /// a formality.
    ///
    /// **The answer lives beside the stub**, and is flipped in the change that
    /// replaces it: nobody has to remember a second list when the protocol is
    /// finally written, which is the failure this repository keeps paying for.
    ///
    /// [`ExtractionDispatcher::implemented_protocols`]: crate::application::extraction::ExtractionDispatcher::implemented_protocols
    fn is_implemented(&self) -> bool;
}
