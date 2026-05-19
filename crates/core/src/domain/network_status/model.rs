//! Network status domain model.
//!
//! A snapshot of the indexer's link to the Solana chain: the latest
//! observed slot and the round-trip latency of the `getSlot` call
//! that produced it.
//!
//! This is a pure domain type — no persistence, no serialization
//! concerns. The persistence layer maps it to/from the singleton
//! `network_status` row; the API layer maps it to its own response
//! DTO.

use chrono::{DateTime, Utc};

/// A point-in-time health snapshot of the chain link.
///
/// Produced by the indexer (which owns the RPC connection), persisted
/// as the single row of `network_status`, and read back by the API
/// for the dashboard's "Solana Live" panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Latest Solana slot observed by the indexer.
    ///
    /// Slots are `u64` on-chain. Kept as `u64` in the domain; the
    /// persistence layer handles the `u64 <-> i64/BIGINT` cast.
    pub slot: u64,

    /// Round-trip latency of the `getSlot` RPC call, in milliseconds.
    pub rpc_latency_ms: u32,

    /// When the indexer recorded this snapshot.
    pub observed_at: DateTime<Utc>,
}
