//! A transaction as the extractors see it, whatever source delivered it: the
//! adapters that fill it live in `yog-indexer`, and owe it the contract below.

use solana_pubkey::Pubkey;

use crate::domain::TransactionPosition;

/// A transaction as the extraction pipeline sees it, whatever delivered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnChainTransaction {
    /// Signature, block time, slot, and index within the slot when the source has one.
    pub position: TransactionPosition,

    /// Every inner-instruction payload, by outer instruction then emission order.
    /// ⚠️ The order is the persisted `event_index`: another order inserts duplicates
    /// under a new numbering on re-ingestion, and fails nothing.
    pub inner_instructions: Vec<InnerInstructionPayload>,
}

/// One inner instruction, reduced to what event decoding needs.
/// ⚠️ Adapters only ever widen what they keep: dropping a payload of the filtered
/// program renumbers every event after it. A narrowing is a migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerInstructionPayload {
    /// Program the instruction was addressed to.
    pub program_id: Pubkey,

    /// Raw instruction data, already decoded from the source's encoding.
    pub data: Vec<u8>,
}
