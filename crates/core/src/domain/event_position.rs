use chrono::{DateTime, Utc};
use solana_signature::Signature;

/// Where an event sits in the chain — the coordinates that make it unique and
/// orderable, independent of what the event says.
///
/// Carried through translation and stamped onto every domain event, whatever
/// its protocol. It exists as one type rather than five loose parameters
/// because the five are only meaningful together: they are one coordinate.
///
/// # Why a signature is not enough
///
/// A transaction routed across several pools emits one event per hop, all
/// under the same signature and the same `timestamp` (which has second
/// granularity, from `blockTime`). Keying events on `(signature, timestamp)`
/// therefore collapses every hop but one — measured at 3,4 % to 8,0 % of a
/// pool's swaps depending on how central it is to routing. `event_index`
/// is what tells them apart.
///
/// # `event_index` numbers raw payloads, not decoded events
///
/// It is the position of the emission among the program's Anchor self-CPI
/// inner instructions, **including those whose discriminator we do not
/// decode**. Numbering only the events we recognise would shift every index
/// already stored the day one more discriminator is implemented, turning a
/// replay into a source of duplicates instead of a no-op.
///
/// The contract that makes this hold is the filter in
/// `extract_anchor_event_cpis`: protocolar (a self-CPI to the program), never
/// tied to the discriminators we happen to know. See its doc-comment.
///
/// # `transaction_index` is `None` today — the provider's doing, not the API's
///
/// The field is `#[serde(default)]` on the Solana type: a `getTransaction`
/// response *may* carry it, and some RPCs do — 10 of the 27 mainnet fixtures in
/// `core/tests/fixtures/damm_v2/` have one. The provider this project ingests
/// from does not: **measured 31 August 2026, 136 of 136 rows of
/// `pool_current_state` have `last_transaction_index IS NULL`** — not most, all.
///
/// So the order reachable today is `(slot, event_index)`: total within a
/// transaction and between slots, but not between two transactions of the same
/// slot. The field is here so a source that carries the index natively — the
/// gRPC/Geyser update, or an RPC that returns it — closes that gap without a
/// schema change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventPosition {
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Slot the transaction landed in. Coarser than a timestamp, but exact:
    /// the ledger's own ordering unit.
    pub slot: u64,
    /// Position of the transaction within its slot. `None` on the
    /// `getTransaction` ingestion path (see above).
    pub transaction_index: Option<u32>,
    /// Position of the emission among the program's self-CPI inner
    /// instructions in this transaction, starting at 0.
    pub event_index: u16,
}

/// The part of an [`EventPosition`] that every event of one transaction
/// shares. Read once per transaction, then stamped onto each event with
/// [`TransactionPosition::at`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionPosition {
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub slot: u64,
    pub transaction_index: Option<u32>,
}

impl TransactionPosition {
    /// Locate the `event_index`-th event of this transaction.
    pub fn at(&self, event_index: u16) -> EventPosition {
        EventPosition {
            signature: self.signature,
            timestamp: self.timestamp,
            slot: self.slot,
            transaction_index: self.transaction_index,
            event_index,
        }
    }
}
