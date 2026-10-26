use chrono::{DateTime, Utc};
use solana_signature::Signature;

/// Where an event sits in the chain: what makes it unique and orderable.
/// ⚠️ A signature is not enough: a routed transaction emits one event per hop,
/// under one signature and one second. `event_index` tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventPosition {
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Slot the transaction landed in: the ledger's own ordering unit.
    pub slot: u64,
    /// Position of the transaction within its slot; `None` from our `getTransaction`
    /// provider, so two transactions of one slot cannot be ordered.
    pub transaction_index: Option<u32>,
    /// Position among the transaction's inner instructions addressed to the program, from 0.
    /// ⚠️ Counts every one, `event_cpi` or not: a new decoder must not renumber stored rows.
    pub event_index: u16,
}

/// The part of an [`EventPosition`] every event of one transaction shares.
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
