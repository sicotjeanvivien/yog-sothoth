use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

/// An admin **re-pacing a farm reward slot**.
///
/// Changes the length of a funding window, which changes the emission rate
/// every subsequent funding will compute (`rate = amount / duration`). It does
/// not re-rate the window already running.
///
/// Product angle: a duration stretched without fresh funding *dilutes* the
/// farm — same tokens spread thinner, lower yield per LP. Read alongside
/// [`crate::domain::MeteoraDammV2FundRewardEvent`], whose Q64.64 rate this
/// event silently reprices going forward.
///
/// Admin-gated (pool creator, or an operator holding the matching permission).
/// Layout from the cp-amm source; no on-chain fixture captured for this event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeteoraDammV2UpdateRewardDurationEvent {
    pub pool_address: Pubkey,
    pub signature: Signature,
    pub timestamp: DateTime<Utc>,
    pub reward_index: u8,
    /// Window length before the change, in seconds.
    pub old_reward_duration: u64,
    /// Window length after the change, in seconds.
    pub new_reward_duration: u64,
}
