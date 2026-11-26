use solana_pubkey::Pubkey;

use crate::domain::{MeteoraDammV2RewardChange, Protocol};

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

    pub fn pool_address(&self) -> Pubkey {
        match self {
            Self::MeteoraDammV2(e) => e.pool_address(),
        }
    }
}
