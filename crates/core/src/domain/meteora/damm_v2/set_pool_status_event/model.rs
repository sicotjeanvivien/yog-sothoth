use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A pool's status flag being changed (e.g. enabled / disabled).
///
/// `status` is the raw on-chain status byte — stored uninterpreted; the
/// meaning of each value is a cp-amm concern decoded by consumers if needed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2SetPoolStatusEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain — see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub status: u8,
}
