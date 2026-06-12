use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// LP closing a position on a pool.
///
/// Marks the end of a position's life: the position account is torn down
/// on-chain. Any remaining liquidity or fees are withdrawn through separate
/// events before the close. Same shape as
/// [`crate::domain::MeteoraDammV2CreatePositionEvent`] — paired with it,
/// these two delimit a position's lifespan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2ClosePositionEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub position_nft_mint: Pubkey,
}
