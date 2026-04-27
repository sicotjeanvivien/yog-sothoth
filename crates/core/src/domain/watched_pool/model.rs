use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;

use crate::domain::Protocol;

/// A pool configured for indexing.
#[derive(Debug, Clone)]
pub struct WatchedPool {
    /// On-chain address of the AMM pool.
    pub pool_address: Pubkey,
    /// AMM protocol.
    pub protocol: Protocol,
    /// The pool: to watch or not to watch
    pub active: bool,
    /// Timestamp when the pool was added
    pub added_at: DateTime<Utc>,
    /// note explaining how the pool watched
    pub note: Option<String>,
}
