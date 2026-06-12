use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// LP opening a new position on a pool.
///
/// A position is an NFT-backed liquidity slot: `position_nft_mint` is the
/// mint of the NFT that represents ownership, `position` is the PDA holding
/// the position state. A freshly created position is empty — liquidity only
/// arrives through a subsequent [`crate::domain::MeteoraDammV2LiquidityEvent`]
/// (add). The event therefore carries no token amounts and no reserves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2CreatePositionEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub owner: Pubkey,
    pub position: Pubkey,
    pub position_nft_mint: Pubkey,
}
