use solana_pubkey::Pubkey;

use crate::{
    CoreResult,
    application::extraction::{ExtractionOutcome, OnChainTransaction},
};

/// One protocol's extraction of domain events from an [`OnChainTransaction`].
/// ⚠️ Per-event failures go into `unknown` or `failures`, never a panic; `Err` is
/// for a malformed transaction as a whole.
pub trait EventExtractor: Send + Sync {
    /// Program ID this extractor decodes events from.
    fn program_id(&self) -> Pubkey;

    /// Extract every domain event the transaction emitted for this protocol.
    fn extract_events(&self, tx: &OnChainTransaction) -> CoreResult<ExtractionOutcome>;

    /// Whether this extractor actually extracts anything.
    /// ⚠️ No default: subscribing to a stub pays a firehose for nothing, so each
    /// extractor answers beside its own body.
    fn is_implemented(&self) -> bool;
}
