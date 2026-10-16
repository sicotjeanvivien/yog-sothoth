//! A snapshot of **the chain**, not of our ingestion: the latest slot and the
//! latency of the `getSlot` that read it, over an endpoint independent of
//! ingestion. Ingestion's own health is `FreshnessStatus`; the two fail apart.

use chrono::{DateTime, Utc};

/// How far along the chain is, as the indexer's reporter last read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Latest Solana slot the reporter saw.
    pub slot: u64,

    /// Round-trip latency of the `getSlot` RPC call, in milliseconds.
    pub rpc_latency_ms: u32,

    /// When the reporter recorded this snapshot.
    pub observed_at: DateTime<Utc>,
}
