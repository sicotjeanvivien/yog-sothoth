use crate::domain::{MeteoraDammV2ClaimProtocolFeeEvent, Protocol};

/// A pool's protocol fees claimed, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum ProtocolFeeClaimEvent {
    MeteoraDammV2(MeteoraDammV2ClaimProtocolFeeEvent),
}

impl ProtocolFeeClaimEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}
