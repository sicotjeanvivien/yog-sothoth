use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// An admin **opening a farming reward slot** on a DAMM v2 pool.
///
/// A pool carries a fixed number of reward slots addressed by `reward_index`;
/// each streams one `reward_mint` token to in-range LPs at a constant rate.
/// This event declares the slot: which token, who may fund it, and how long a
/// funding window lasts (`reward_duration`, in seconds).
///
/// Opening a slot distributes nothing by itself — the tokens and the emission
/// rate arrive with [`crate::domain::MeteoraDammV2FundRewardEvent`], usually in
/// the same transaction. Product angle: this is the "a new farm just launched"
/// marker, the earliest on-chain signal that incentivised liquidity is coming.
///
/// `funder` is the wallet authorised to deposit rewards; `creator` is the one
/// that opened the slot. They are commonly the same wallet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2InitializeRewardEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub reward_mint: Pubkey,
    pub funder: Pubkey,
    pub creator: Pubkey,
    pub reward_index: u8,
    /// Length of a funding window, in seconds (e.g. 604800 = 7 days).
    pub reward_duration: u64,
}
