use crate::domain::{MeteoraDammV2InitializePoolEvent, Protocol};

/// A pool's creation, under its protocol's own event.
#[derive(Debug, Clone)]
pub enum PoolCreationEvent {
    MeteoraDammV2(MeteoraDammV2InitializePoolEvent),
}

impl PoolCreationEvent {
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
        }
    }
}
