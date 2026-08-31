//! The shape in which a transaction reaches the extraction pipeline.
//!
//! `core` has no I/O, and it must not name a transport either: a transaction
//! may arrive as a JSON-RPC `getTransaction` response today and as a Yellowstone
//! protobuf update tomorrow. [`TransactionView`] is what the two have in common,
//! and it is the only thing the extractors see. Each source gets an adapter that
//! fills it — [`super::rpc`] is the one that exists today.
//!
//! It carries exactly what extraction needs, and nothing else: the coordinate
//! that locates an event ([`TransactionPosition`]) and the material to decode
//! (the inner-instruction payloads). No account keys, no logs, no balances —
//! adding a field here means the extractors gained a dependency on the source.

use solana_pubkey::Pubkey;

use crate::domain::TransactionPosition;

/// A transaction as the extraction pipeline sees it, whatever delivered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionView {
    /// Where the transaction sits in the chain — signature, block time, slot,
    /// and its index within the slot when the source provides one.
    pub position: TransactionPosition,

    /// Every inner-instruction payload of the transaction, **in the order the
    /// block puts them in**.
    ///
    /// # ⚠️ This order is a contract, not a convenience
    ///
    /// The position of a payload in this vector — after filtering on the
    /// emitting program — becomes the `event_index` persisted with the event,
    /// and `event_index` is part of the unique key of every event table
    /// (`(signature, event_index, timestamp)`). Rows already in the database
    /// were numbered from this order.
    ///
    /// An adapter that returns the payloads in another order does not fail:
    /// re-ingesting those transactions inserts duplicates under the new
    /// numbering, and the old rows stay, unreachable and wrong. So every
    /// adapter owes this vector the same order — grouped by the outer
    /// instruction they belong to, in ascending order of that instruction's
    /// index, and in emission order within a group.
    ///
    /// The same reasoning governs *which* payloads make it in: see
    /// [`InnerInstructionPayload`].
    pub inner_instructions: Vec<InnerInstructionPayload>,
}

/// One inner instruction, reduced to what event decoding needs.
///
/// # ⚠️ Which payloads belong here is frozen for the same reason
///
/// An adapter must be permissive: every inner instruction it can represent
/// belongs in [`TransactionView::inner_instructions`], regardless of the
/// program that emitted it or of how many accounts it references. Deciding
/// "is this really an event" is the job of the decoder downstream
/// ([`super::decode_anchor_event_cpi`], which checks the Anchor tag).
///
/// Make an adapter stricter — drop a payload it keeps today — and every event
/// after the dropped one in its transaction shifts down by one, with the silent
/// duplication described on [`TransactionView::inner_instructions`]. So: only
/// ever *widen*. A genuine narrowing is a migration (renumber, or version the
/// column), not an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InnerInstructionPayload {
    /// Program the instruction was addressed to.
    pub program_id: Pubkey,

    /// Raw instruction data, already decoded from whatever encoding the source
    /// used to ship it.
    pub data: Vec<u8>,
}
