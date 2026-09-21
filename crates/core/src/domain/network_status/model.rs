//! Network status domain model.
//!
//! A snapshot of **the chain**, not of our link to it: the latest observed
//! slot and the round-trip latency of the `getSlot` call that produced it,
//! read over an endpoint chosen for being independent of ingestion. Whether
//! *our* ingestion is keeping up is a different question with a different
//! answer — `FreshnessStatus`, derived from the last event written to the
//! database — and the dashboard shows the two side by side because they fail
//! separately.
//!
//! This is a pure domain type — no persistence, no serialization
//! concerns. The persistence layer maps it to/from the singleton
//! `network_status` row; the API layer maps it to its own response
//! DTO.

use chrono::{DateTime, Utc};

/// A point-in-time reading of how far along the chain is.
///
/// Recorded by the indexer's reporter — which opens its own client on its own
/// endpoint, sharing neither with ingestion — persisted as the single row of
/// `network_status`, and read back by the API for the dashboard's "Solana
/// Live" panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Latest Solana slot the reporter saw.
    ///
    /// Slots are `u64` on-chain. Kept as `u64` in the domain; the
    /// persistence layer handles the `u64 <-> i64/BIGINT` cast.
    pub slot: u64,

    /// Round-trip latency of the `getSlot` RPC call, in milliseconds.
    pub rpc_latency_ms: u32,

    /// When the reporter recorded this snapshot.
    pub observed_at: DateTime<Utc>,
}
