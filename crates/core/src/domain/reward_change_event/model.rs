use chrono::{DateTime, Utc};
use solana_pubkey::Pubkey;
use solana_signature::Signature;

use crate::domain::{
    MeteoraDammV2FundRewardEvent, MeteoraDammV2InitializeRewardEvent,
    MeteoraDammV2UpdateRewardDurationEvent, MeteoraDammV2UpdateRewardFunderEvent,
    MeteoraDammV2WithdrawDeadLiquidityRewardEvent, MeteoraDammV2WithdrawIneligibleRewardEvent,
    Protocol,
};

/// A pool's reward set up, funded, changed or withdrawn, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum RewardChangeEvent {
    MeteoraDammV2(MeteoraDammV2RewardChange),
}

impl RewardChangeEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}

/// A DAMM v2 reward change: one of the six events that change a pool's reward.
#[derive(Debug, Clone)]
pub enum MeteoraDammV2RewardChange {
    Initialize(MeteoraDammV2InitializeRewardEvent),
    Fund(MeteoraDammV2FundRewardEvent),
    DurationChange(MeteoraDammV2UpdateRewardDurationEvent),
    FunderChange(MeteoraDammV2UpdateRewardFunderEvent),
    IneligibleWithdrawal(MeteoraDammV2WithdrawIneligibleRewardEvent),
    DeadLiquidityWithdrawal(MeteoraDammV2WithdrawDeadLiquidityRewardEvent),
}

impl MeteoraDammV2RewardChange {
    /// The key its list is sorted on: its instant, signature and index in the transaction.
    pub fn key(&self) -> (DateTime<Utc>, Signature, u16) {
        match self {
            Self::Initialize(e) => (e.timestamp, e.signature, e.event_index),
            Self::Fund(e) => (e.timestamp, e.signature, e.event_index),
            Self::DurationChange(e) => (e.timestamp, e.signature, e.event_index),
            Self::FunderChange(e) => (e.timestamp, e.signature, e.event_index),
            Self::IneligibleWithdrawal(e) => (e.timestamp, e.signature, e.event_index),
            Self::DeadLiquidityWithdrawal(e) => (e.timestamp, e.signature, e.event_index),
        }
    }

    pub fn slot(&self) -> u64 {
        match self {
            Self::Initialize(e) => e.slot,
            Self::Fund(e) => e.slot,
            Self::DurationChange(e) => e.slot,
            Self::FunderChange(e) => e.slot,
            Self::IneligibleWithdrawal(e) => e.slot,
            Self::DeadLiquidityWithdrawal(e) => e.slot,
        }
    }

    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::Initialize(e) => e.pool_address,
            Self::Fund(e) => e.pool_address,
            Self::DurationChange(e) => e.pool_address,
            Self::FunderChange(e) => e.pool_address,
            Self::IneligibleWithdrawal(e) => e.pool_address,
            Self::DeadLiquidityWithdrawal(e) => e.pool_address,
        }
    }
}
