use solana_pubkey::Pubkey;

use crate::domain::{MeteoraDammV2ClaimRewardEvent, Protocol};

/// A position's reward claimed from its pool, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum RewardClaimEvent {
    MeteoraDammV2(MeteoraDammV2ClaimRewardEvent),
}

impl RewardClaimEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }

    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::MeteoraDammV2(e) => e.pool_address,
        }
    }
}
