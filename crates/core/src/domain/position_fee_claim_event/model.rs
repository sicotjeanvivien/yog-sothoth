use crate::domain::{MeteoraDammV2ClaimPositionFeeEvent, Protocol};

/// A position's fees claimed from its pool, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum PositionFeeClaimEvent {
    MeteoraDammV2(MeteoraDammV2ClaimPositionFeeEvent),
}

impl PositionFeeClaimEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}
