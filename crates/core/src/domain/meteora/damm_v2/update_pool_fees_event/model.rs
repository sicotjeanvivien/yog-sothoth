use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// An operator updates a pool's fee parameters, kept as the raw blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2UpdatePoolFeesEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain, see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub operator: Pubkey,
    /// Raw bytes of the on-chain `UpdatePoolFeesParameters`, decoded where they are read.
    pub params_raw: Vec<u8>,
}
