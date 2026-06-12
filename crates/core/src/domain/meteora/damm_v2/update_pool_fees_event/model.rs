use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// A pool's fee parameters being updated by an operator.
///
/// "voie C": the new fee parameters are captured as a raw, undecoded byte
/// blob (`params_raw` — the trailing `UpdatePoolFeesParameters` of the wire
/// event). The fee schedule is interpreted later by dedicated work, reading
/// these stored bytes. `operator` is the account that performed the update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2UpdatePoolFeesEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub operator: Pubkey,
    /// Raw, undecoded bytes of the on-chain `UpdatePoolFeesParameters`.
    pub params_raw: Vec<u8>,
}
