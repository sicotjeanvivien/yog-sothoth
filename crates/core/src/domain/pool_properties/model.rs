use crate::domain::{MeteoraDammV2PoolProperties, MeteoraDlmmPoolProperties, Protocol};

/// A pool's stored satellite row, one variant per protocol.
/// Partial where [`crate::domain::PoolAccountProperties`] is total: a row, not a read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolProperties {
    MeteoraDammV2(MeteoraDammV2PoolProperties),
    MeteoraDlmm(MeteoraDlmmPoolProperties),
}

impl PoolProperties {
    /// The protocol these properties belong to.
    pub fn protocol(&self) -> Protocol {
        match self {
            Self::MeteoraDammV2(_) => Protocol::MeteoraDammV2,
            Self::MeteoraDlmm(_) => Protocol::MeteoraDlmm,
        }
    }
}
