use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// An admin **transferring the right to fund a farm reward slot**.
///
/// Moves no tokens and does not touch the emission rate — it only changes which
/// wallet may call `fund_reward` on this `reward_index`, and which wallet
/// receives reclaimed rewards.
///
/// Product angle: a change of funder is a change of who is paying for the
/// incentive — a hand-over of the farm, and a useful provenance trace when
/// reading a pool's incentive history.
///
/// Admin-gated (pool creator, or an operator holding the matching permission).
/// Layout from the cp-amm source; no on-chain fixture captured for this event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2UpdateRewardFunderEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    /// Position in the chain — see [`crate::domain::EventPosition`].
    pub slot: u64,
    pub transaction_index: Option<u32>,
    pub event_index: u16,
    pub reward_index: u8,
    pub old_funder: Pubkey,
    pub new_funder: Pubkey,
}
